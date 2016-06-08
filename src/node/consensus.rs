use time::{get_time};

use super::super::crypto::{Hash};
use super::super::messages::{ConsensusMessage, Propose, Prevote, Precommit, Commit, Message};
use super::NodeContext;

pub struct ConsensusService;

pub trait ConsensusHandler {
    fn handle(&mut self, ctx: &mut NodeContext, msg: ConsensusMessage) {
        // Ignore messages from previous height
        if msg.height() < ctx.state.height() + 1 {
            return
        }

        // Queued messages from future height
        if msg.height() > ctx.state.height() + 1 {
            ctx.state.add_queued(msg);
            return
        }

        match ctx.state.public_key_of(msg.validator()) {
            // Incorrect signature of message
            Some(public_key) => if !msg.verify() {
                return
            },
            // Incorrect validator id
            None => return
        }

        match message {
            ConsensusMessage::Propose(msg) => {
                // Check prev_hash
                if propose.prev_hash() != ctx.state.prev_hash() {
                    return
                }

                // Check leader
                if propose.validator() != ctx.state.leader(propose.round()) {
                    return
                }

                // TODO: check time
                // TODO: check that transactions are not commited yet

                self.handle_propose(ctx, msg)
            },
            ConsensusMessage::Prevote(msg) => self.handle_prevote(ctx, msg),
            ConsensusMessage::Precommit(msg) => self.handle_precommit(ctx, msg),
            ConsensusMessage::Commit(msg) => self.handle_commit(ctx, msg),
        }
    }

    fn handle_propose(&mut self, ctx: &mut NodeContext, propose: Propose) {
        // Add propose
        let hash = propose.hash();
        let added = ctx.state.add_propose(hash, &propose);

        if added {
            // TODO: Check that we "have block"
            for h in propose.transactions() {
                if !ctx.state.tx_pool().contains_key(h) {
                    panic!("unknown transactions into propose");
                }
            }

            self.have_block(ctx, hash);
        }
    }

    fn have_block(&mut self, ctx: &mut NodeContext, hash: Hash) {
        // Send prevote
        if ctx.state.locked_round() == 0 &&
           ctx.state.propose(hash).round() == ctx.state.round() {
            self.send_prevote(ctx, block_hash);
        }

        // Lock to propose
        if ctx.state.has_majority_prevotes(ctx.state.round(), hash) &&
           ctx.state.locked_round() < ctx.state.round() {
            self.lock(ctx, hash);
        }
    }

    fn handle_prevote(&mut self, ctx: &mut NodeContext, prevote: Prevote) {
        // TODO: what is the reason of handling and storing
        //  prevotes for previous rounds?

        // Add prevote
        let has_consensus = ctx.state.add_prevote(&prevote);

        // Lock to propose
        if has_consensus &&
           prevote.round() == ctx.state.round() &&
           ctx.state.locked_round() < ctx.state.round() {
            self.lock(ctx, prevote.block_hash());
        }
    }

    fn lock(&mut self, ctx: &mut NodeContext, block_hash: Hash) {
        // Change lock
        ctx.state.lock(block_hash);

        // Execute block and get state hash
        let state_hash = match ctx.state.state_hash(block_hash) {
            Some(state_hash) => state_hash,
            None => self.execute(ctx, hash)
        };

        // Send precommit
        self.send_precommit(ctx, block_hash, state_hash);

        // Commit if has consensus
        if ctx.state.has_majority_precommits(ctx.state.round(),
                                             block_hash,
                                             state_hash) {
            self.commit(ctx, ctx.state.round(), block_hash);
        }
    }

    fn handle_precommit(&mut self, ctx: &mut NodeContext, msg: Precommit) {
        // Add precommit
        let has_consensus = ctx.state.add_precommit(&msg);

        if has_consensus {
            // Execute block and get state hash
            let state_hash = match ctx.state.state_hash(msg.block_hash()) {
                Some(state_hash) => state_hash,
                None => self.execute(ctx, hash)
            };

            if state_hash != msg.state_hash() {
                panic!("We are fucked up...");
            }

            self.commit(ctx, msg.round(), msg.block_hash());
        }
    }

    fn commit(&mut self, ctx: &mut NodeContext,
              round: Round, hash: Hash, changes: &Changes) {
        // Merge changes into storage
        ctx.storage.merge(changes);

        // Update state to new height
        ctx.state.new_height();

        // TODO: remove old transactions

        // Generate new transactions
        for tx in (&mut ctx.tx_generator).take(100) {
            ctx.state.add_tx(tx.hash(), tx);
        }

        // Send commit
        self.send_commit(ctx, ctx.state.height() - 1, round, hash);

        // Handle queued messages
        for msg in ctx.state.queue() {
            self.handle(ctx, msg);
        }

        // Send propose
        if self.is_leader(ctx) {
            self.send_propose(ctx);
        }

        // Add timeout for first round
        ctx.add_timeout();
    }

    fn handle_commit(&mut self, _: &mut NodeContext, _: Commit) {
    }

    fn handle_timeout(&mut self, ctx: &mut NodeContext, timeout: Timeout) {
        if timeout.height != ctx.state.height() {
            return;
        }

        if timeout.round != ctx.state.round() {
            return;
        }

        // Update state to new round
        ctx.state.new_round();

        // TODO: check that we have +2/3 prevotes in this round for some
        // block and lock to it.

        // Send prevote if we are locked or propose if we are leader
        // TODO: check that we have propose for new round and prevote it
        if let Some(hash) = self.locked_propose() {
            self.send_prevote(ctx, hash);
        } else if self.is_leader(ctx) {
            self.send_propose(ctx);
        }

        // Add timeout for this round
        self.context.add_timeout();
    }

    fn is_leader(&self, ctx: &NodeContext) -> bool {
        ctx.state.leader(ctx.state.round()) == ctx.id
    }

    fn execute(&mut self, ctx: &mut NodeContext, hash: Hash) -> Hash {
        let fork = Fork::new(ctx.storage);

        fork.put_block(msg);

        let changes = fork.changes();
        let hash = changes.hash();
        ctx.add_changes(hash, changes);
        hash
    }

    fn send_propose(&mut self, ctx: &mut NodeContext) {
        let propose = Propose::new(ctx.id,
                                   ctx.state.height(),
                                   ctx.state.round(),
                                   get_time(),
                                   ctx.storage.prev_hash(),
                                   &ctx.state.transactions(),
                                   &ctx.secret_key);
        ctx.broadcast(&propose);

        let hash = propose.hash();
        ctx.state.add_propose(propose., propose);

        // Send prevote
        self.send_prevote(hash);
    }

    fn send_prevote(&mut self, ctx: &mut NodeContext, block_hash: Hash) {
        // TODO: check that we are not send prevote for this round
        let prevote = Prevote::new(ctx.id,
                                   ctx.height(),
                                   ctx.round(),
                                   block_hash,
                                   &ctx.secret_key);
        ctx.state.add_prevote(&prevote);
        ctx.broadcast(prevote);
    }

    fn send_precommit(&mut self, ctx: &mut NodeContext,
                      block_hash: Hash, state_hash: Hash) {
        // TODO: check that we are not send precommit for this round
        let precommit = Precommit::new(ctx.id,
                                       ctx.state.height(),
                                       ctx.state.round(),
                                       block_hash,
                                       state_hash,
                                       &ctx.secret_key);
        ctx.broadcast(&precommit);
        ctx.state.add_precommit(&precommit);
    }

    fn send_commit(&mut self, ctx: &mut NodeContext,
                   height: Height, round: Round, block_hash: Hash) {
        // Send commit
        let commit = Commit::new(ctx.state.id,
                                 height,
                                 round,
                                 block_hash,
                                 &ctx.secret_key);
        ctx.broadcast(commit);
    }
}

impl ConsensusHandler for ConsensusService {
    // default implementation
}
