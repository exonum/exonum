use time::{get_time};

use super::super::messages::{ConsensusMessage, Propose, Prevote, Precommit, Commit, Message};
use super::NodeContext;

pub struct ConsensusService;

pub trait ConsensusHandler {
    fn handle(&mut self, ctx: &mut NodeContext, message: ConsensusMessage) {
        match message {
            ConsensusMessage::Propose(message) => self.handle_propose(ctx, message),
            ConsensusMessage::Prevote(message) => self.handle_prevote(ctx, message),
            ConsensusMessage::Precommit(message) => self.handle_precommit(ctx, message),
            ConsensusMessage::Commit(message) => self.handle_commit(ctx, message),
        }
    }

    fn handle_propose(&mut self, ctx: &mut NodeContext, propose: Propose) {
        // debug!("recv propose");
        if propose.height() > ctx.state.height() + 1 {
            ctx.state.queue(ConsensusMessage::Propose(propose.clone()));
            return;
        }

        if propose.height() < ctx.state.height() + 1 {
            if !ctx.byzantine {
                // info!("=== Invalid block proposed, ignore ===")
            }
            return;
        }

        if propose.prev_hash() != ctx.state.prev_hash() {
            return;
        }

        if propose.validator() != ctx.state.leader(propose.round()) {
            return;
        }

        let (hash, queue) = ctx.state.add_propose(propose.round(),
                                                   propose.clone());

        // debug!("send prevote");
        let prevote = Prevote::new(ctx.id,
                                   propose.height(),
                                   propose.round(),
                                   &hash,
                                   &ctx.secret_key);
        ctx.broadcast(prevote.raw().clone());
        self.handle_prevote(ctx, prevote);

        for message in queue {
            self.handle(ctx, message);
        }
    }

    fn handle_prevote(&mut self, ctx: &mut NodeContext, prevote: Prevote) {
        // debug!("recv prevote");
        if prevote.height() > ctx.state.height() + 1 {
            ctx.state.queue(ConsensusMessage::Prevote(prevote.clone()));
            return;
        }

        if prevote.height() < ctx.state.height() + 1 {
            return;
        }

        let has_consensus = ctx.state.add_prevote(prevote.round(),
                                                   prevote.hash(),
                                                   prevote.clone());

        if has_consensus {
            ctx.state.lock_round(prevote.round());
            // debug!("send precommit");
            let precommit = Precommit::new(ctx.id,
                                           prevote.height(),
                                           prevote.round(),
                                           prevote.hash(),
                                           &ctx.secret_key);
            ctx.broadcast(precommit.raw().clone());
            self.handle_precommit(ctx, precommit);
        }
    }

    fn handle_precommit(&mut self, ctx: &mut NodeContext, precommit: Precommit) {
        // debug!("recv precommit");
        if precommit.height() > ctx.state.height() + 1 {
            ctx.state.queue(ConsensusMessage::Precommit(precommit.clone()));
            return;
        }

        if precommit.height() < ctx.state.height() + 1 {
            return;
        }

        let has_consensus = ctx.state.add_precommit(precommit.round(),
                                                    precommit.hash(),
                                                    precommit.clone());

        if has_consensus {
            let queue = ctx.state.new_height(precommit.hash().clone());

            for tx in (&mut ctx.tx_generator).take(100) {
                ctx.state.add_tx(tx);
            }

            // info!("Commit block #{}", ctx.state.height());
            if self.is_leader(ctx) {
                self.make_propose(ctx);
            } else {
                // debug!("send commit");
                // let commit = Commit::new(precommit.height(),
                //                          precommit.hash(),
                //                          &ctx.public_key,
                //                          &ctx.secret_key);
                // ctx.broadcast(commit.clone());
                // self.handle_commit(commit);
            }
            for message in queue {
                self.handle(ctx, message);
            }
            ctx.add_timeout();
        }
    }

    fn handle_commit(&mut self, _: &mut NodeContext, _: Commit) {
        // debug!("recv commit");
        // nothing
    }

    fn is_leader(&self, ctx: &NodeContext) -> bool {
        ctx.state.leader(ctx.state.round()) == ctx.id
    }

    fn make_propose(&mut self, ctx: &mut NodeContext) {
        // debug!("send propose");
        // FIXME: remove this sheet
        // ::std::thread::sleep(::std::time::Duration::from_millis(ctx.propose_timeout as u64));
        let height = if ctx.byzantine {
            // info!("=== Propose invalid block ===");
            0
        } else {
            ctx.state.height() + 1
        };
        let propose = Propose::new(ctx.id,
                                   height,
                                   ctx.state.round(),
                                   get_time(),
                                   ctx.state.prev_hash(),
                                   &ctx.secret_key);
        ctx.broadcast(propose.raw().clone());
        self.handle_propose(ctx, propose);
    }
}

impl ConsensusHandler for ConsensusService {
    // default implementation
}
