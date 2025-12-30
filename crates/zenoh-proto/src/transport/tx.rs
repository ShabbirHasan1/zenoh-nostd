use std::u16;

use crate::{
    exts::QoS,
    fields::Reliability,
    msgs::NetworkMessage,
    transport::state::{StateResponse, TransportState},
};

#[derive(Debug)]
pub struct TransportTx<Buff> {
    pub(crate) streamed: bool,
    tx: Buff,

    last_batch_size: u16,
    next_sn: u32,

    last_qos: Option<QoS>,
    last_reliability: Option<Reliability>,
}

impl<Buff> TransportTx<Buff> {
    pub fn new(tx: Buff) -> Self
    where
        Buff: AsRef<[u8]>,
    {
        Self {
            last_batch_size: tx.as_ref().len() as u16,
            tx,
            streamed: false,
            next_sn: 0,
            last_qos: None,
            last_reliability: None,
        }
    }

    pub(crate) fn sync(&mut self, state: &TransportState) -> &mut Self {
        if self.next_sn == 0 {
            self.next_sn = *state.sn();
        }

        self
    }

    pub fn write<'a, 'b>(
        &'a mut self,
        msgs: impl Iterator<Item = NetworkMessage<'b>>,
    ) -> impl Iterator<Item = &'a [u8]>
    where
        Buff: AsMut<[u8]>,
    {
        let streamed = self.streamed;
        let mut buffer = self.tx.as_mut();
        let batch_size = core::cmp::min(self.last_batch_size as usize, buffer.len());

        let mut msgs = msgs.peekable();

        let reliability = &mut self.last_reliability;
        let qos = &mut self.last_qos;
        let sn = &mut self.next_sn;

        core::iter::from_fn(move || {
            let batch_size = core::cmp::min(batch_size as usize, buffer.len());
            let batch = &mut buffer[..batch_size];

            if streamed && batch_size < 2 {
                return None;
            }

            let mut writer = &mut batch[if streamed { 2 } else { 0 }..];
            let start = writer.len();

            let mut length = 0;
            while let Some(msg) = msgs.peek() {
                if msg.z_encode(&mut writer, reliability, qos, sn).is_ok() {
                    length = start - writer.len();
                    msgs.next();
                } else {
                    break;
                }
            }

            if length == 0 {
                return None;
            }

            if streamed {
                let l = (length as u16).to_be_bytes();
                batch[..2].copy_from_slice(&l);
            }

            let (ret, remain) =
                core::mem::take(&mut buffer).split_at_mut(length + if streamed { 2 } else { 0 });
            buffer = remain;

            Some(&ret[..])
        })
    }
}
