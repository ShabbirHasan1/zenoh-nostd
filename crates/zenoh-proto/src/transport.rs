use crate::transport::{rx::TransportRx, state::TransportState, tx::TransportTx};

mod scope;
mod state;

mod rx;
mod tx;

#[derive(Debug)]
pub struct Transport<Buff> {
    tx: TransportTx<Buff>,
    rx: TransportRx<Buff>,

    state: TransportState,
}

impl<Buff> Transport<Buff> {
    pub fn new(buff: Buff) -> Self
    where
        Buff: Clone + AsRef<[u8]>,
    {
        Self {
            state: TransportState::codec().with_batch_size(buff.as_ref().len() as u16),

            tx: TransportTx::new(buff.clone()),
            rx: TransportRx::new(buff),
        }
    }

    pub fn codec(mut self) -> Self {
        self.state = self.state.into_codec();
        self
    }

    pub fn listen(mut self) -> Self {
        self.state = self.state.into_listen();
        self
    }

    pub fn connect(mut self) -> Self {
        self.state = self.state.into_connect();
        self
    }

    pub fn streamed(mut self) -> Self {
        self.rx.streamed = true;
        self.tx.streamed = true;
        self
    }
}

// impl<Buff> TransportTx<Buff> {
//     pub fn new(tx: Buff) -> Self {
//         Self {
//             tx,
//             streamed: false,
//         }
//     }

//     pub fn write<'a, 'b>(
//         &'a mut self,
//         msgs: impl Iterator<Item = NetworkMessage<'b>>,
//     ) -> impl Iterator<Item = &'a [u8]>
//     where
//         Buff: AsMut<[u8]>,
//     {
//         let streamed = self.streamed;
//         let mut buffer = self.tx.as_mut();
//         let batch_size = core::cmp::min(self.batch_size as usize, buffer.len());

//         let mut msgs = msgs.peekable();

//         let reliability = &mut self.reliability;
//         let qos = &mut self.qos;
//         let sn = &mut self.sn;

//         core::iter::from_fn(move || {
//             let batch_size = core::cmp::min(batch_size as usize, buffer.len());
//             let batch = &mut buffer[..batch_size];

//             if streamed && batch_size < 2 {
//                 return None;
//             }

//             let mut writer = &mut batch[if streamed { 2 } else { 0 }..];
//             let start = writer.len();

//             let mut length = 0;
//             while let Some(msg) = msgs.peek() {
//                 if msg.z_encode(&mut writer, reliability, qos, sn).is_ok() {
//                     length = start - writer.len();
//                     msgs.next();
//                 } else {
//                     break;
//                 }
//             }

//             if length == 0 {
//                 return None;
//             }

//             if streamed {
//                 let l = (length as u16).to_be_bytes();
//                 batch[..2].copy_from_slice(&l);
//             }

//             let (ret, remain) =
//                 core::mem::take(&mut buffer).split_at_mut(length + if streamed { 2 } else { 0 });
//             buffer = remain;

//             Some(&ret[..])
//         })
//     }

//     pub fn interact<'a>(&mut self, s: &mut TransportLocal<'a>) -> &'a [u8]
//     where
//         Buff: AsMut<[u8]>,
//     {
//         if s.state.is_waiting() {
//             return &[];
//         }

//         &[]
//     }
// }

// #[derive(Debug)]
// pub struct Transport<Buff> {
//     tx: TransportTx<Buff>,
//     rx: TransportRx<Buff>,

//     state: TransportState,
// }

// use crate::{TransportError, ZReadable, exts::*, fields::*, msgs::*};

// #[derive(Debug, PartialEq, Default)]
// pub struct TransportState {
//     inner: TransportStateInner,
// }

// impl TransportState {
//     pub fn local(&mut self) -> core::result::Result<TransportLocal<'_>, TransportError> {
//         let storage = match self.inner {
//             TransportStateInner::Codec => TransportLocalInner::Codec,
//             TransportStateInner::Opened => TransportLocalInner::Opened,
//             _ => return Err(TransportError::IncompleteState),
//         };

//         Ok(TransportLocal {
//             state: self,
//             inner: storage,
//         })
//     }

//     pub fn codec() -> Self {
//         Self {
//             inner: TransportStateInner::Codec,
//         }
//     }

//     pub fn listen() -> Self {
//         Self {
//             inner: TransportStateInner::WaitingInitSyn,
//         }
//     }

//     pub fn connect() -> Self {
//         Self {
//             inner: TransportStateInner::SendingInitSyn,
//         }
//     }

//     pub fn is_sending(&self) -> bool {
//         self.inner.is_sending()
//     }

//     pub fn is_waiting(&self) -> bool {
//         self.inner.is_waiting()
//     }

//     pub fn is_opened(&self) -> bool {
//         self.inner.is_opened()
//     }
// }

// #[allow(dead_code)]
// #[derive(Debug, PartialEq, Default)]
// enum TransportStateInner {
//     #[default]
//     /// No state handling:
//     Codec,

//     /// States for `listen` mode.
//     WaitingInitSyn,
//     SendingInitAck,
//     WaitingOpenSyn,
//     SendingOpenAck,

//     /// States for `connect` mode.
//     SendingInitSyn,
//     WaitingInitAck,
//     SendingOpenSyn,
//     WaitingOpenAck,

//     /// Opened state after handshake.
//     Opened,
// }

// impl TransportStateInner {
//     fn is_sending(&self) -> bool {
//         matches!(
//             self,
//             TransportStateInner::SendingInitSyn { .. }
//                 | TransportStateInner::SendingInitAck { .. }
//                 | TransportStateInner::SendingOpenSyn { .. }
//                 | TransportStateInner::SendingOpenAck { .. }
//         )
//     }

//     fn is_waiting(&self) -> bool {
//         matches!(
//             self,
//             TransportStateInner::WaitingInitSyn { .. }
//                 | TransportStateInner::WaitingInitAck { .. }
//                 | TransportStateInner::WaitingOpenSyn { .. }
//                 | TransportStateInner::WaitingOpenAck { .. }
//         )
//     }

//     pub fn is_opened(&self) -> bool {
//         matches!(self, TransportStateInner::Opened)
//     }
// }

// #[derive(Debug, PartialEq)]
// pub struct TransportLocal<'a> {
//     state: &'a mut TransportState,
//     inner: TransportLocalInner<'a>,
// }

// impl<'a> TransportLocal<'a> {
//     fn handle(&mut self, msg: TransportMessage<'a>) {
//         if matches!(self.inner, TransportLocalInner::Codec) {
//             return;
//         }

//         match msg {
//             TransportMessage::InitSyn(syn) => {
//                 self.inner = TransportLocalInner::SendingInitAck { syn };
//             }
//             _ => {}
//         }
//     }
// }

// #[allow(dead_code)]
// #[derive(Debug, PartialEq, Default)]
// enum TransportLocalInner<'a> {
//     #[default]
//     /// No state handling:
//     Codec,

//     /// States for `listen` mode.
//     WaitingInitSyn,
//     SendingInitAck {
//         syn: InitSyn<'a>,
//     },
//     WaitingOpenSyn,
//     SendingOpenAck,

//     /// States for `connect` mode.
//     SendingInitSyn,
//     WaitingInitAck,
//     SendingOpenSyn {
//         cookie: &'a [u8],
//     },
//     WaitingOpenAck,

//     /// Opened state after handshake.
//     Opened,
// }

// #[derive(Debug)]
// pub struct TransportRx<Buff> {
//     rx: Buff,
//     cursor: usize,
//     streamed: bool,

//     frame: Option<FrameHeader>,
// }

// #[derive(Debug)]
// pub struct TransportTx<Buff> {
//     tx: Buff,

//     batch_size: u16,
//     streamed: bool,

//     sn: u32,
//     reliability: Option<Reliability>,
//     qos: Option<QoS>,
// }

// #[derive(Debug)]
// pub struct Transport<Buff> {
//     pub tx: TransportTx<Buff>,
//     pub rx: TransportRx<Buff>,

//     pub state: TransportState,
// }

// impl<Buff> Transport<Buff> {
//     pub fn new(buff: Buff) -> Self
//     where
//         Buff: Clone + AsRef<[u8]>,
//     {
//         Self {
//             tx: TransportTx {
//                 tx: buff.clone(),
//                 batch_size: buff.as_ref().len().min(u16::MAX as usize) as u16,
//                 streamed: false,

//                 sn: 0,
//                 qos: None,
//                 reliability: None,
//             },
//             rx: TransportRx {
//                 rx: buff,
//                 cursor: 0,
//                 streamed: false,
//                 frame: None,
//             },
//             state: TransportState::codec(),
//         }
//     }

//     pub fn codec(mut self) -> Self {
//         self.state = TransportState::codec();
//         self
//     }

//     pub fn listen(mut self) -> Self {
//         self.state = TransportState::listen();
//         self
//     }

//     pub fn connect(mut self) -> Self {
//         self.state = TransportState::connect();
//         self
//     }

//     pub fn batch_size(mut self, batch_size: u16) -> Self {
//         self.tx.batch_size = batch_size;
//         self
//     }

//     pub fn streamed(mut self) -> Self {
//         self.tx.streamed = true;
//         self.rx.streamed = true;
//         self
//     }
// }

// impl<Buff> TransportRx<Buff> {
//     fn next<'a>(
//         reader: &mut &'a [u8],
//         local: &mut TransportLocal<'a>,
//         frame: &mut Option<FrameHeader>,
//     ) -> Option<NetworkMessage<'a>> {
//         if !reader.can_read() {
//             return None;
//         }

//         let header = reader
//             .read_u8()
//             .expect("reader should not be empty at this stage");

//         macro_rules! decode {
//             ($ty:ty) => {
//                 match <$ty as $crate::ZBodyDecode>::z_body_decode(reader, header) {
//                     Ok(msg) => msg,
//                     Err(e) => {
//                         crate::error!(
//                             "Failed to decode message of type {}: {}. Skipping the rest of the message - {}",
//                             core::any::type_name::<$ty>(),
//                             e,
//                             crate::zctx!()
//                         );

//                         return None;
//                     }
//                 }
//             };
//         }

//         let ack = header & 0b0010_0000 != 0;
//         let net = frame.is_some();
//         let ifinal = header & 0b0110_0000 == 0;
//         let id = header & 0b0001_1111;

//         if let Some(msg) = match id {
//             InitAck::ID if ack => Some(TransportMessage::InitAck(decode!(InitAck))),
//             InitSyn::ID => Some(TransportMessage::InitSyn(decode!(InitSyn))),
//             OpenAck::ID if ack => Some(TransportMessage::OpenAck(decode!(OpenAck))),
//             OpenSyn::ID => Some(TransportMessage::OpenSyn(decode!(OpenSyn))),
//             Close::ID => Some(TransportMessage::Close(decode!(Close))),
//             KeepAlive::ID => Some(TransportMessage::KeepAlive(decode!(KeepAlive))),
//             _ => None,
//         } {
//             frame.take();
//             local.handle(msg);
//             return Self::next(reader, local, frame);
//         }

//         let reliability = frame.as_ref().map(|f| f.reliability);
//         let qos = frame.as_ref().map(|f| f.qos);

//         // TODO! negociate `sn` and expect increasing `sn` in frames

//         let body = match id {
//             FrameHeader::ID => {
//                 let header = decode!(FrameHeader);
//                 frame.replace(header);
//                 return Self::next(reader, local, frame);
//             }
//             Push::ID if net => NetworkBody::Push(decode!(Push)),
//             Request::ID if net => NetworkBody::Request(decode!(Request)),
//             Response::ID if net => NetworkBody::Response(decode!(Response)),
//             ResponseFinal::ID if net => NetworkBody::ResponseFinal(decode!(ResponseFinal)),
//             InterestFinal::ID if net && ifinal => {
//                 NetworkBody::InterestFinal(decode!(InterestFinal))
//             }
//             Interest::ID if net => NetworkBody::Interest(decode!(Interest)),
//             Declare::ID if net => NetworkBody::Declare(decode!(Declare)),
//             _ => {
//                 crate::error!(
//                     "Unrecognized message header: {:08b}. Skipping the rest of the message - {}",
//                     header,
//                     crate::zctx!()
//                 );
//                 return None;
//             }
//         };

//         Some(NetworkMessage {
//             reliability: reliability.expect("Should be a frame. Something went wrong."),
//             qos: qos.expect("Should be a frame. Something went wrong."),
//             body,
//         })
//     }

//     pub fn flush<'a>(
//         &'a mut self,
//         local: &mut TransportLocal<'a>,
//     ) -> impl Iterator<Item = NetworkMessage<'a>>
//     where
//         Buff: AsRef<[u8]>,
//     {
//         let rx = self.rx.as_ref();
//         let mut reader = &rx[..self.cursor];
//         let frame = &mut self.frame;

//         core::iter::from_fn(move || Self::next(&mut reader, local, frame))
//     }

//     pub fn feed(&mut self, local: &mut TransportLocal, data: &[u8])
//     where
//         Buff: AsMut<[u8]>,
//     {
//         if local.state.is_sending() {
//             return;
//         }

//         let rx = self.rx.as_mut();
//         let rx = &mut rx[self.cursor..];

//         let len = data.len().min(rx.len());
//         rx[..len].copy_from_slice(&data[..len]);
//         self.cursor += len;
//     }

//     pub fn feed_exact(
//         &mut self,
//         local: &mut TransportLocal,
//         len: usize,
//         mut data: impl FnMut(&mut [u8]),
//     ) where
//         Buff: AsMut<[u8]>,
//     {
//         if local.state.is_sending() {
//             return;
//         }

//         let rx = self.rx.as_mut();
//         let rx = &mut rx[self.cursor..];

//         let len = len.min(rx.len());
//         data(&mut rx[..len]);
//         self.cursor += len;
//     }

//     pub fn feed_with(
//         &mut self,
//         local: &mut TransportLocal,
//         mut data: impl FnMut(&mut [u8]) -> usize,
//     ) where
//         Buff: AsMut<[u8]>,
//     {
//         if local.state.is_sending() {
//             return;
//         }

//         let rx = self.rx.as_mut();
//         let rx = &mut rx[self.cursor..];

//         if self.streamed {
//             let mut lbuf = [0u8; 2];
//             data(&mut lbuf);
//             let l = u16::from_be_bytes(lbuf) as usize;
//             data(&mut rx[..l]);
//             self.cursor += l;
//         } else {
//             let l = data(&mut rx[..]);
//             self.cursor += l;
//         }
//     }
// }

// impl<Buff> TransportTx<Buff> {
//     pub fn write<'a, 'b>(
//         &'a mut self,
//         msgs: impl Iterator<Item = NetworkMessage<'b>>,
//     ) -> impl Iterator<Item = &'a [u8]>
//     where
//         Buff: AsMut<[u8]>,
//     {
//         let streamed = self.streamed;
//         let mut buffer = self.tx.as_mut();
//         let batch_size = core::cmp::min(self.batch_size as usize, buffer.len());

//         let mut msgs = msgs.peekable();

//         let reliability = &mut self.reliability;
//         let qos = &mut self.qos;
//         let sn = &mut self.sn;

//         core::iter::from_fn(move || {
//             let batch_size = core::cmp::min(batch_size as usize, buffer.len());
//             let batch = &mut buffer[..batch_size];

//             if streamed && batch_size < 2 {
//                 return None;
//             }

//             let mut writer = &mut batch[if streamed { 2 } else { 0 }..];
//             let start = writer.len();

//             let mut length = 0;
//             while let Some(msg) = msgs.peek() {
//                 if msg.z_encode(&mut writer, reliability, qos, sn).is_ok() {
//                     length = start - writer.len();
//                     msgs.next();
//                 } else {
//                     break;
//                 }
//             }

//             if length == 0 {
//                 return None;
//             }

//             if streamed {
//                 let l = (length as u16).to_be_bytes();
//                 batch[..2].copy_from_slice(&l);
//             }

//             let (ret, remain) =
//                 core::mem::take(&mut buffer).split_at_mut(length + if streamed { 2 } else { 0 });
//             buffer = remain;

//             Some(&ret[..])
//         })
//     }

//     pub fn interact<'a>(&mut self, s: &mut TransportLocal<'a>) -> &'a [u8]
//     where
//         Buff: AsMut<[u8]>,
//     {
//         if s.state.is_waiting() {
//             return &[];
//         }

//         &[]
//     }
// }
