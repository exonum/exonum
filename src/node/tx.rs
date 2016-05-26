use super::super::messages::{TxMessage};
use super::{NodeContext};


pub struct TxService;

pub trait TxHandler {
    fn handle(&mut self, ctx: &mut NodeContext, message: TxMessage) {
        // FIXME: make sure that it is new transaction
        // FIXME: validate transaction signature
        ctx.state.add_tx(message.hash(), message);
    }

}

impl TxHandler for TxService {
    // default implementation
}
