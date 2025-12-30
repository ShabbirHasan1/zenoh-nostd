use crate::{
    Transport, TransportError,
    transport::{
        rx::TransportRx,
        state::{StateRequest, StateResponse, TransportState},
        tx::TransportTx,
    },
};

#[derive(Debug)]
pub struct TransportStateScoped<'a> {
    state: &'a mut TransportState,
    pending: Option<StateResponse<'a>>,
}

impl<'a> TransportStateScoped<'a> {
    pub(crate) fn is_codec(&self) -> bool {
        self.state.is_codec()
    }

    pub(crate) fn process(
        &mut self,
        request: Option<StateRequest<'a>>,
    ) -> core::result::Result<(), TransportError> {
        let response = self.state.process(request)?;
        self.pending = response;
        Ok(())
    }
}

#[derive(Debug)]
pub struct TransportScope<'a, Buff> {
    pub tx: &'a mut TransportTx<Buff>,
    pub rx: &'a mut TransportRx<Buff>,
    pub state: TransportStateScoped<'a>,
}

impl<Buff> Transport<Buff> {
    pub fn scope(&mut self) -> TransportScope<'_, Buff> {
        TransportScope {
            tx: self.tx.sync(&self.state),
            rx: self.rx.sync(&self.state),
            state: TransportStateScoped {
                state: &mut self.state,
                pending: None,
            },
        }
    }
}
