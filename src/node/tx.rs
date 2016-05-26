use super::super::messages::{TxMessage};
use super::{NodeContext};


pub struct Tx;

pub trait TxService {
    fn handle(&mut self, ctx: &mut NodeContext, message: TxMessage) {
        unimplemented!();
    }

}

impl TxService for Tx {
    // default implementation
}
