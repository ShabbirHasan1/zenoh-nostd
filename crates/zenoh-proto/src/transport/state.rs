use core::{time::Duration, u16, u32};

use sha3::{
    Shake128,
    digest::{ExtendableOutput, Update, XofReader},
};

use crate::{
    TransportError,
    fields::{BatchSize, Bits, Field, Resolution, ZenohIdProto},
    msgs::{InitAck, InitIdentifier, InitResolution, InitSyn, OpenAck, OpenSyn, TransportMessage},
};

#[derive(Debug, Default, PartialEq)]
pub(crate) enum State {
    /// Special state avoiding handshake strategy
    #[default]
    Codec,

    /// State entry point in listening mode
    WaitingInitSyn,
    WaitingOpenSyn {
        zid: ZenohIdProto,
    },

    /// State entry point in connecting mode
    Connecting,
    WaitingInitAck,
    WaitingOpenAck {
        zid: ZenohIdProto,
    },

    Opened {
        zid: ZenohIdProto,
        lease: Duration,
        sn: u32,
    },
    Closed,
}

#[derive(Debug, PartialEq)]
pub(crate) struct TransportState {
    zid: ZenohIdProto,
    batch_size: u16,
    resolution: Resolution,
    lease: Duration,
    sn: u32,

    pub inner: State,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            zid: ZenohIdProto::default(),
            batch_size: u16::MAX,
            resolution: Resolution::default(),
            sn: u32::MIN,
            lease: Duration::from_secs(10),

            inner: State::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct StateRequest<'a>(pub TransportMessage<'a>);
#[derive(Debug, PartialEq)]
pub(crate) struct StateResponse<'a>(pub TransportMessage<'a>);

impl TransportState {
    pub fn codec() -> Self {
        Self {
            inner: State::Codec,
            ..Default::default()
        }
    }

    pub fn into_codec(mut self) -> Self {
        self.inner = State::Codec;
        self
    }

    pub fn listen() -> Self {
        Self {
            inner: State::WaitingInitSyn,
            ..Default::default()
        }
    }

    pub fn into_listen(mut self) -> Self {
        self.inner = State::WaitingInitSyn;
        self
    }

    pub fn connect() -> Self {
        Self {
            inner: State::Connecting,
            ..Default::default()
        }
    }

    pub fn into_connect(mut self) -> Self {
        self.inner = State::Connecting;
        self
    }

    pub fn is_codec(&self) -> bool {
        matches!(self.inner, State::Codec)
    }

    pub fn zid(&self) -> &ZenohIdProto {
        &self.zid
    }

    pub fn batch_size(&self) -> &u16 {
        &self.batch_size
    }

    pub fn sn(&self) -> &u32 {
        &self.sn
    }

    pub fn resolution(&self) -> &Resolution {
        &self.resolution
    }

    pub fn lease(&self) -> &Duration {
        &self.lease
    }

    pub fn opened(&self) -> bool {
        matches!(self.inner, State::Opened { .. })
    }

    pub fn with_zid(mut self, zid: ZenohIdProto) -> Self {
        if matches!(
            self.inner,
            State::Codec | State::Connecting | State::WaitingInitSyn
        ) {
            self.zid = zid;
        }

        self
    }

    pub fn with_batch_size(mut self, batch_size: u16) -> Self {
        if matches!(
            self.inner,
            State::Codec | State::Connecting | State::WaitingInitSyn
        ) {
            self.batch_size = batch_size;
        }

        self
    }

    pub fn with_resolution(mut self, resolution: Resolution) -> Self {
        if matches!(
            self.inner,
            State::Codec | State::Connecting | State::WaitingInitSyn
        ) {
            self.resolution = resolution;
        }

        self
    }

    pub fn with_lease(mut self, lease: Duration) -> Self {
        if matches!(
            self.inner,
            State::Codec | State::Connecting | State::WaitingInitSyn
        ) {
            self.lease = lease;
        }

        self
    }

    pub fn process<'a>(
        &mut self,
        request: Option<StateRequest<'a>>,
    ) -> core::result::Result<Option<StateResponse<'a>>, TransportError> {
        if matches!(self.inner, State::Codec) {
            crate::trace!("Transport in Codec Mode. Skipping request");
            return Ok(None);
        }

        if matches!(self.inner, State::Closed) {
            crate::error!("Transport has previously been closed");
            crate::zbail!(@log TransportError::TransportIsClosed);
        }

        let response = if let Some(request) = request {
            match request.0 {
                // If Opened, close the transport
                TransportMessage::Close(_) => match &self.inner {
                    State::Opened { zid, .. } => {
                        crate::debug!("Received Close on transport {:?} -> {:?}", self.zid, zid);
                        self.inner = State::Closed;
                        None
                    }
                    _ => crate::zbail!(@log TransportError::StateCantHandle),
                },
                // If Opened, log it
                TransportMessage::KeepAlive(_) => match &self.inner {
                    State::Opened { zid, .. } => {
                        crate::debug!(
                            "Received KeepAlive on transport {:?} -> {:?}",
                            self.zid,
                            zid
                        );

                        None
                    }
                    _ => crate::zbail!(@log TransportError::StateCantHandle),
                },
                // Receiving an InitSyn should not engage resource creation. We simply reply with a cookie
                // equals to the buffer so that we can use it later. TODO: the cookie should be cyphered but
                // it's not that easy in a no-std no-alloc way
                TransportMessage::InitSyn(syn) => match &self.inner {
                    State::WaitingInitSyn => {
                        crate::debug!(
                            "Received InitSyn on transport {:?} -> NEW!({:?})",
                            self.zid,
                            syn.identifier.zid
                        );

                        // We're not supposed to store it but there is no allocation so I do it anyway
                        self.inner = State::WaitingOpenSyn {
                            zid: syn.identifier.zid,
                        };

                        Some(StateResponse(TransportMessage::InitAck(InitAck {
                            identifier: InitIdentifier {
                                zid: self.zid.clone(),
                                ..Default::default()
                            },
                            resolution: InitResolution {
                                resolution: self.resolution,
                                batch_size: BatchSize(self.batch_size),
                            },
                            cookie: &[], // TODO: cookie authentication.
                            ..Default::default()
                        })))
                    }
                    _ => crate::zbail!(@log TransportError::StateCantHandle),
                },
                // When receiving an InitAck we should negotiate the batch_size/resolution and send the `sn` we will ue
                TransportMessage::InitAck(ack) => match &self.inner {
                    State::WaitingInitAck => {
                        crate::debug!(
                            "Received InitAck on transport {:?} -> {:?}",
                            self.zid,
                            ack.identifier.zid
                        );

                        self.batch_size = self.batch_size.min(ack.resolution.batch_size.0);
                        self.resolution = {
                            let mut res = Resolution::default();
                            let i_fsn_res = ack.resolution.resolution.get(Field::FrameSN);
                            let m_fsn_res = self.resolution.get(Field::FrameSN);
                            if i_fsn_res > m_fsn_res {
                                crate::zbail!(@log TransportError::InvalidAttribute);
                            }
                            res.set(Field::FrameSN, i_fsn_res);
                            let i_rid_res = ack.resolution.resolution.get(Field::RequestID);
                            let m_rid_res = self.resolution.get(Field::RequestID);
                            if i_rid_res > m_rid_res {
                                crate::zbail!(@log TransportError::InvalidAttribute);
                            }
                            res.set(Field::RequestID, i_rid_res);
                            res
                        };
                        self.sn = {
                            let mut hasher = Shake128::default();
                            hasher.update(&self.zid.as_le_bytes()[..self.zid.size()]);
                            hasher.update(
                                &ack.identifier.zid.as_le_bytes()[..ack.identifier.zid.size()],
                            );
                            let mut array = (0 as u32).to_le_bytes();
                            hasher.finalize_xof().read(&mut array);
                            u32::from_le_bytes(array)
                                & match self.resolution.get(Field::FrameSN) {
                                    Bits::U8 => u8::MAX as u32 >> 1,
                                    Bits::U16 => u16::MAX as u32 >> 2,
                                    Bits::U32 => u32::MAX as u32 >> 4,
                                    Bits::U64 => u64::MAX as u32 >> 1,
                                }
                        };

                        self.inner = State::WaitingOpenAck {
                            zid: ack.identifier.zid,
                        };

                        Some(StateResponse(TransportMessage::OpenSyn(OpenSyn {
                            lease: self.lease.clone(),
                            sn: self.sn,
                            cookie: ack.cookie,
                            ..Default::default()
                        })))
                    }
                    _ => crate::zbail!(@log TransportError::StateCantHandle),
                },
                TransportMessage::OpenSyn(syn) => match &self.inner {
                    State::WaitingOpenSyn { zid } => {
                        // We are not supposed to use a stored zid, but we're supposed
                        // to decrypt the cookie to get access to the data.
                        crate::debug!("Received OpenSyn on transport {:?} -> {:?}", self.zid, zid);

                        self.inner = State::Opened {
                            zid: zid.clone(),
                            lease: syn.lease,
                            sn: syn.sn,
                        };

                        Some(StateResponse(TransportMessage::OpenAck(OpenAck {
                            lease: self.lease,
                            sn: self.sn,
                            ..Default::default()
                        })))
                    }
                    _ => crate::zbail!(@log TransportError::StateCantHandle),
                },
                TransportMessage::OpenAck(ack) => match &self.inner {
                    State::WaitingOpenAck { zid } => {
                        crate::debug!("Received OpenAck on transport {:?} -> {:?}", self.zid, zid);

                        self.inner = State::Opened {
                            zid: zid.clone(),
                            lease: ack.lease,
                            sn: ack.sn,
                        };

                        None
                    }
                    _ => crate::zbail!(@log TransportError::StateCantHandle),
                },
            }
        } else {
            match self.inner {
                State::Connecting => {
                    self.inner = State::WaitingInitAck;

                    Some(StateResponse(TransportMessage::InitSyn(InitSyn {
                        identifier: InitIdentifier {
                            zid: self.zid.clone(),
                            ..Default::default()
                        },
                        resolution: InitResolution {
                            resolution: self.resolution,
                            batch_size: BatchSize(self.batch_size),
                        },
                        ..Default::default()
                    })))
                }
                _ => None,
            }
        };

        Ok(response)
    }
}

#[test]
fn test_transport_state() {
    // Codec
    let mut state = TransportState::codec();

    let q = StateRequest(TransportMessage::InitAck(InitAck::default()));
    let a = state
        .process(Some(q))
        .expect("Codec Mode should never fail as it doesn't process anything");
    assert_eq!(a, None);
    assert!(!state.opened());

    // Listen
    let mut state = TransportState::listen();

    let q = StateRequest(TransportMessage::InitSyn(InitSyn::default()));
    let a = state
        .process(Some(q))
        .expect("Only error possible is when state is not capable of handling the request, this shouldnot happen");
    if !matches!(a, Some(StateResponse(TransportMessage::InitAck(_)))) {
        panic!()
    }

    let q = StateRequest(TransportMessage::InitSyn(InitSyn::default()));
    state.process(Some(q)).expect_err("Expected error");

    let q = StateRequest(TransportMessage::OpenSyn(OpenSyn::default()));
    let a = state
        .process(Some(q))
        .expect("Only error possible is when state is not capable of handling the request, this shouldnot happen");
    if !matches!(a, Some(StateResponse(TransportMessage::OpenAck(_)))) {
        panic!()
    }
    assert!(state.opened());

    // Connect
    let mut state = TransportState::connect();

    let q = StateRequest(TransportMessage::InitSyn(InitSyn::default()));
    state
        .process(Some(q))
        .expect_err("Expected error because should not be able to handle init_syn");

    let q = None;
    let a = state
        .process(q)
        .expect("Only error possible is when state is not capable of handling the request, this shouldnot happen");
    if !matches!(a, Some(StateResponse(TransportMessage::InitSyn(_)))) {
        panic!()
    }

    let q = StateRequest(TransportMessage::OpenSyn(OpenSyn::default()));
    state.process(Some(q)).expect_err("Expected error");

    let q = StateRequest(TransportMessage::InitAck(InitAck::default()));
    let a = state
        .process(Some(q))
        .expect("Only error possible is when state is not capable of handling the request, this shouldnot happen");
    if !matches!(a, Some(StateResponse(TransportMessage::OpenSyn(_)))) {
        panic!()
    }

    let q = StateRequest(TransportMessage::OpenAck(OpenAck::default()));
    let a = state
        .process(Some(q))
        .expect("Only error possible is when state is not capable of handling the request, this shouldnot happen");
    assert_eq!(a.is_some(), false);
    assert!(state.opened());
}
