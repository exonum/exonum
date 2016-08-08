use super::super::messages::{BasicMessage, Connect, Status, Message};
use super::{NodeContext, RequestData};


pub struct BasicService;

pub trait BasicHandler {
    fn handle(&mut self, ctx: &mut NodeContext, message: BasicMessage) {
        match message {
            BasicMessage::Connect(msg) => self.handle_connect(ctx, msg),
            BasicMessage::Status(msg) => self.handle_status(ctx, msg),
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

    fn handle_status(&self, ctx: &mut NodeContext, msg: Status) {
        info!("recv status");
        // Handle message from future height
        if msg.height() > ctx.state.height() {
            // Check validator height info
            // FIXME: make sure that validator id < validator count
            if ctx.state.validator_height(msg.validator()) >= msg.height() {
                return;
            }
            // Verify validator if and signature
            match ctx.state.public_key_of(msg.validator()) {
                // Incorrect signature of message
                Some(public_key) => if !msg.verify(&public_key) {
                    return
                },
                // Incorrect validator id
                None => return
            };
            // Update validator height
            ctx.state.set_validator_height(msg.validator(), msg.height());
            // Request commit
            ctx.request(RequestData::Commit, msg.validator());
        }

        // TODO: remove this?
        // // Handle message from current height
        // if msg.height() == ctx.state.height() {
        //     // Request propose or txs
        //     self.request_propose_or_txs(ctx, msg.propose_hash(), msg.validator());

        //     // Request precommits
        //     if !ctx.state.has_majority_precommits(msg.round(),
        //                                           *msg.propose_hash(),
        //                                           *msg.block_hash()) {
        //         let data = RequestData::Precommits(msg.round(),
        //                                           *msg.propose_hash(),
        //                                           *msg.block_hash());
        //         self.request(ctx, data, msg.validator());
        //     }
        // }
    }

    fn handle_status_timeout(&self, ctx: &mut NodeContext) {
        if let Some(hash) = ctx.storage.last_hash().unwrap() {
            info!("send status");
            // Send status
            let status = Status::new(ctx.state.id(),
                                     ctx.state.height(),
                                     &hash,
                                     &ctx.secret_key);
            ctx.broadcast(status.raw());
        }
        ctx.add_status_timeout();
    }
}

impl BasicHandler for BasicService {
    // default implementation
}
