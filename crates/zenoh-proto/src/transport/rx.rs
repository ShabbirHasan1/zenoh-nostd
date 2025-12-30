use crate::{
    ZReadable,
    msgs::*,
    transport::{
        scope::TransportStateScoped,
        state::{State, StateRequest, TransportState},
    },
};

#[derive(Debug)]
pub struct TransportRx<Buff> {
    rx: Buff,
    cursor: usize,
    pub(crate) streamed: bool,

    frame: Option<FrameHeader>,
    next_expected_sn: u32,
}

impl<Buff> TransportRx<Buff> {
    pub fn new(rx: Buff) -> Self {
        Self {
            rx,
            cursor: 0,
            streamed: false,
            frame: None,
            next_expected_sn: 0,
        }
    }

    pub(crate) fn sync(&mut self, state: &TransportState) -> &mut Self {
        if let State::Opened { sn, .. } = &state.inner
            && self.next_expected_sn == 0
        {
            self.next_expected_sn = *sn;
        }

        self
    }

    pub fn feed(&mut self, data: &[u8])
    where
        Buff: AsMut<[u8]>,
    {
        let rx = self.rx.as_mut();
        let rx = &mut rx[self.cursor..];

        let len = data.len().min(rx.len());
        rx[..len].copy_from_slice(&data[..len]);
        self.cursor += len;
    }

    pub fn feed_exact(&mut self, len: usize, mut data: impl FnMut(&mut [u8]))
    where
        Buff: AsMut<[u8]>,
    {
        let rx = self.rx.as_mut();
        let rx = &mut rx[self.cursor..];

        let len = len.min(rx.len());
        data(&mut rx[..len]);
        self.cursor += len;
    }

    pub fn feed_with(&mut self, mut data: impl FnMut(&mut [u8]) -> usize)
    where
        Buff: AsMut<[u8]>,
    {
        let rx = self.rx.as_mut();
        let rx = &mut rx[self.cursor..];

        if self.streamed {
            let mut lbuf = [0u8; 2];
            data(&mut lbuf);
            let l = u16::from_be_bytes(lbuf) as usize;
            data(&mut rx[..l]);
            self.cursor += l;
        } else {
            let l = data(&mut rx[..]);
            self.cursor += l;
        }
    }

    fn read_one<'a>(
        reader: &mut &'a [u8],
        state: &mut TransportStateScoped<'a>,
        last_frame: &mut Option<FrameHeader>,
    ) -> Option<NetworkMessage<'a>> {
        if !reader.can_read() {
            return None;
        }

        let header = reader
            .read_u8()
            .expect("reader should not be empty at this stage");

        macro_rules! decode {
            ($ty:ty) => {
                match <$ty as $crate::ZBodyDecode>::z_body_decode(reader, header) {
                    Ok(msg) => msg,
                    Err(e) => {
                        crate::error!(
                            "Failed to decode message of type {}: {}. Skipping the rest of the message - {}",
                            core::any::type_name::<$ty>(),
                            e,
                            crate::zctx!()
                        );

                        return None;
                    }
                }
            };
        }

        let ack = header & 0b0010_0000 != 0;
        let net = last_frame.is_some();
        let ifinal = header & 0b0110_0000 == 0;
        let id = header & 0b0001_1111;

        if !state.is_codec() {
            if let Some(msg) = match id {
                InitAck::ID if ack => Some(TransportMessage::InitAck(decode!(InitAck))),
                InitSyn::ID => Some(TransportMessage::InitSyn(decode!(InitSyn))),
                OpenAck::ID if ack => Some(TransportMessage::OpenAck(decode!(OpenAck))),
                OpenSyn::ID => Some(TransportMessage::OpenSyn(decode!(OpenSyn))),
                Close::ID => Some(TransportMessage::Close(decode!(Close))),
                KeepAlive::ID => Some(TransportMessage::KeepAlive(decode!(KeepAlive))),
                _ => None,
            } {
                if let Err(e) = state.process(Some(StateRequest(msg))) {
                    crate::error!("Error while processing a TransportMessage. {:?}", e);
                }

                return Self::read_one(reader, state, last_frame);
            }
        }

        let reliability = last_frame.as_ref().map(|f| f.reliability);
        let qos = last_frame.as_ref().map(|f| f.qos);
        let sn = last_frame.as_ref().map(|f| f.sn);

        let body = match id {
            FrameHeader::ID => {
                let header = decode!(FrameHeader);
                if !state.is_codec() {
                    if let Some(sn) = sn {
                        if header.sn <= sn {
                            crate::error!(
                                "Inconsistent `SN` value {}, expected higher than {}",
                                header.sn,
                                sn
                            );
                            return None;
                        } else if header.sn != sn + 1 {
                            crate::debug!("Transport missed {} messages", header.sn - sn - 1);
                        }
                    }
                }

                last_frame.replace(header);
                return Self::read_one(reader, state, last_frame);
            }
            Push::ID if net => NetworkBody::Push(decode!(Push)),
            Request::ID if net => NetworkBody::Request(decode!(Request)),
            Response::ID if net => NetworkBody::Response(decode!(Response)),
            ResponseFinal::ID if net => NetworkBody::ResponseFinal(decode!(ResponseFinal)),
            InterestFinal::ID if net && ifinal => {
                NetworkBody::InterestFinal(decode!(InterestFinal))
            }
            Interest::ID if net => NetworkBody::Interest(decode!(Interest)),
            Declare::ID if net => NetworkBody::Declare(decode!(Declare)),
            _ => {
                crate::error!(
                    "Unrecognized message header: {:08b}. Skipping the rest of the message - {}",
                    header,
                    crate::zctx!()
                );
                return None;
            }
        };

        Some(NetworkMessage {
            reliability: reliability.expect("Should be a frame. Something went wrong."),
            qos: qos.expect("Should be a frame. Something went wrong."),
            body,
        })
    }

    pub fn flush<'a>(
        &'a mut self,
        state: &mut TransportStateScoped<'a>,
    ) -> impl Iterator<Item = NetworkMessage<'a>>
    where
        Buff: AsRef<[u8]>,
    {
        let rx = self.rx.as_ref();
        let mut reader = &rx[..self.cursor];
        let frame = &mut self.frame;

        core::iter::from_fn(move || Self::read_one(&mut reader, state, frame))
    }
}
