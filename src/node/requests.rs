use time::{get_time, Duration};

use super::super::messages::{
    RequestMessage, Message,
    RequestPropose, RequestTransactions, RequestPrevotes,
    RequestPrecommits, RequestCommit, RequestPeers
};
use super::super::storage::{Map};
use super::{NodeContext};



const REQUEST_ALIVE : i64 = 3_000_000_000; // 3 seconds


pub struct RequestService;

pub trait RequestHandler {
    fn handle(&mut self, ctx: &mut NodeContext, msg: RequestMessage) {
        // Request are sended to us
        if msg.to() != ctx.state.id() {
            return;
        }

        // FIXME: we should use some epsilon for checking lifetime < 0
        let lifetime = match (get_time() - msg.time()).num_nanoseconds() {
            Some(nanos) => nanos,
            None => {
                // Incorrect time into message
                return
            }
        };

        // Incorrect time of the request
        if lifetime < 0 || lifetime > REQUEST_ALIVE {
            return;
        }

        match ctx.state.public_key_of(msg.from()) {
            // Incorrect signature of message
            Some(public_key) => if !msg.verify(&public_key) {
                return
            },
            // Incorrect validator id
            None => return
        }

        match msg {
            RequestMessage::Propose(msg) => self.handle_propose(ctx, msg),
            RequestMessage::Transactions(msg) => self.handle_txs(ctx, msg),
            RequestMessage::Prevotes(msg) => self.handle_prevotes(ctx, msg),
            RequestMessage::Precommits(msg) => self.handle_precommits(ctx, msg),
            RequestMessage::Commit(msg) => self.handle_commit(ctx, msg),
            RequestMessage::Peers(msg) => self.handle_peers(ctx, msg),
        }
    }

    fn handle_propose(&mut self, ctx: &mut NodeContext, msg: RequestPropose) {
        if msg.height() > ctx.state.height() {
            return
        }

        let propose = if msg.height() == ctx.state.height() {
            ctx.state.propose(msg.propose_hash()).map(|p| p.message().raw().clone())
        } else {  // msg.height < state.height
            ctx.storage.proposes().get(msg.propose_hash()).unwrap().map(|p| p.raw().clone())
        };

        if let Some(propose) = propose {
            ctx.send_to_validator(msg.from(), &propose);
        }
    }

    fn handle_txs(&mut self, ctx: &mut NodeContext, msg: RequestTransactions) {
        for hash in msg.txs() {
            let tx = ctx.state.transactions().get(hash).map(|tx| tx.clone())
                              .or_else(|| ctx.storage.transactions().get(hash).unwrap());

            if let Some(tx) = tx {
                ctx.send_to_validator(msg.from(), tx.raw());
            }
        }
    }

    fn handle_prevotes(&mut self, ctx: &mut NodeContext, msg: RequestPrevotes) {
        if msg.height() != ctx.state.height() {
            return
        }

        let prevotes = if let Some(prevotes) = ctx.state.prevotes(msg.round(),
                                                                  msg.propose_hash().clone()) {
            prevotes.values().map(|p| p.raw().clone()).collect()
        } else {
            Vec::new()
        };

        for prevote in prevotes {
            ctx.send_to_validator(msg.from(), &prevote);
        }
    }

    fn handle_precommits(&mut self, ctx: &mut NodeContext, msg: RequestPrecommits) {
        if msg.height() > ctx.state.height() {
            return
        }

        let precommits = if msg.height() == ctx.state.height() {
            if let Some(precommits) = ctx.state.precommits(msg.round(),
                                                           msg.propose_hash().clone(),
                                                           msg.block_hash().clone()) {
                precommits.values().map(|p| p.raw().clone()).collect()
            } else {
                Vec::new()
            }
        } else {  // msg.height < state.height
            if let Some(precommits) = ctx.storage.precommits(msg.block_hash()).iter().unwrap() {
                precommits.iter().map(|p| p.raw().clone()).collect()
            } else {
                Vec::new()
            }
        };

        for precommit in precommits {
            ctx.send_to_validator(msg.from(), &precommit);
        }
    }

    fn handle_commit(&mut self, ctx: &mut NodeContext, msg: RequestCommit) {
        if msg.height() >= ctx.state.height() {
            return
        }

        let block_hash = ctx.storage.heights().get(msg.height()).unwrap().unwrap();

        let precommits = if let Some(precommits) = ctx.storage.precommits(&block_hash).iter().unwrap() {
            precommits.iter().map(|p| p.raw().clone()).collect()
        } else {
            Vec::new()
        };

        for precommit in precommits {
            ctx.send_to_validator(msg.from(), &precommit);
        }
    }

    fn handle_peers(&mut self, ctx: &mut NodeContext, msg: RequestPeers) {
        // TODO
    }
}

impl RequestHandler for RequestService {
    // default implementation
}
