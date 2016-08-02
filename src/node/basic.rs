use super::super::messages::{BasicMessage, Connect, Message};
use super::super::events::{Reactor};
use super::{NodeContext};


pub struct BasicService;

pub trait BasicHandler {
    fn handle(&mut self, ctx: &mut NodeContext, message: BasicMessage) {
        match message {
            BasicMessage::Connect(message) => self.handle_connect(ctx, message)
        }
    }

    fn handle_connect(&mut self, ctx: &mut NodeContext, message: Connect) {
        let public_key = message.pub_key().clone();
        let address = message.addr();
        if ctx.state.add_peer(public_key, address) {
            // TODO: reduce double sending of connect message
            info!("Establish connection with {}", address);
            let message = Connect::new(&ctx.public_key,
                                       ctx.events.address().clone(),
                                       ctx.events.get_time(),
                                       &ctx.secret_key);
            ctx.send_to_addr(&address, message.raw());
        }
    }
}

impl BasicHandler for BasicService {
    // default implementation
}
