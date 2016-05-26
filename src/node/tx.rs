use super::super::messages::{TxMessage};
use super::{NodeContext};


pub struct TxService;

pub trait TxHandler {
    fn handle(&mut self, ctx: &mut NodeContext, message: TxMessage) {
        unimplemented!();
    }

}

impl TxHandler for TxService {
    // default implementation
}
