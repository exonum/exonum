use time::{get_time};

use super::super::events::{Events, Event, Timeout, EventsConfiguration};
use super::super::crypto::{Hash};
use super::super::messages::{ConsensusMessage, Propose, Prevote, Precommit, Commit, Message};
use super::super::storage::{Fork, Patch};
use super::{NodeContext, Round, Height};

pub struct ConsensusService;

pub trait ConsensusHandler {
    fn handle(&self, ctx: &mut NodeContext, msg: ConsensusMessage) {
        // Ignore messages from previous height
        if msg.height() < ctx.state.height() + 1 {
            return
        }

        // Queued messages from future height or round
        if msg.height() > ctx.state.height() + 1 ||
           msg.round() > ctx.state.round() {
            ctx.state.add_queued(msg);
            return
        }

        match ctx.state.public_key_of(msg.validator()) {
            // Incorrect signature of message
            Some(public_key) => if !msg.verify(&public_key) {
                return
            },
            // Incorrect validator id
            None => return
        }

        match msg {
            ConsensusMessage::Propose(msg) => {
                // Check prev_hash
                if msg.prev_hash() != &ctx.storage.prev_hash() {
                    return
                }

                // Check leader
                if msg.validator() != ctx.state.leader(msg.round()) {
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

    fn handle_propose(&self, ctx: &mut NodeContext, propose: Propose) {
        // Add propose
        let hash = propose.hash();
        let added = ctx.state.add_propose(hash, &propose);

        if added {
            // TODO: Temp (Check that we "have block")
            // for h in propose.transactions() {
            //     if !ctx.state.transactions().contains_key(h) {
            //         panic!("unknown transactions into propose");
            //     }
            // }

            self.have_block(ctx, hash);
        }
    }

    fn have_block(&self, ctx: &mut NodeContext, hash: Hash) {
        let round = ctx.state.proposal(&hash).unwrap().round();

        // Send prevote
        if ctx.state.locked_round() == 0 {
            self.send_prevote(ctx, round, &hash);
        }

        // Lock to propose
        let start_round = ::std::cmp::max(ctx.state.locked_round() + 1,
                                          round);
        for round in start_round ... ctx.state.round() {
            if ctx.state.has_majority_prevotes(round, hash) {
                self.lock(ctx, round, hash);
            }
        }

        // FIXME: Commit if we have +2/3 precommits?
        // for round in propose.round() ... ctx.state.round() {
        //     if ctx.state.has_majority_precommits(round, hash) {
        //         self.commit
        //     }
        // }

        //     self.lock(ctx, hash);
        // }
    }

    fn handle_prevote(&self, ctx: &mut NodeContext, prevote: Prevote) {
        // Add prevote
        let has_consensus = ctx.state.add_prevote(&prevote);

        // Lock to propose
        if has_consensus && ctx.state.locked_round() < prevote.round() {
            let hash = prevote.propose_hash();
            // FIXME: проверка что у нас есть все транзакции
            if ctx.state.proposal(hash).is_some() {
                self.lock(ctx, prevote.round(), *hash);
            }
        }
    }

    fn lock(&self, ctx: &mut NodeContext,
            round: Round, propose_hash: Hash) {
        // Change lock
        ctx.state.lock(round, propose_hash);

        // Send precommit
        if !ctx.state.have_incompatible_prevotes() {
            // Execute block and get state hash
            let block_hash = self.execute(ctx, &propose_hash);
            self.send_precommit(ctx, round, &propose_hash, &block_hash);
            // Commit if has consensus
            if ctx.state.has_majority_precommits(round,
                                                 propose_hash,
                                                 block_hash) {
                self.commit(ctx, round, &propose_hash);
                return
            }
        }

        // Send prevotes
        for round in ctx.state.locked_round() + 1 ... ctx.state.round() {
            if !ctx.state.have_prevote(round) {
                self.send_prevote(ctx, round, &propose_hash);
                if ctx.state.has_majority_prevotes(round, propose_hash) {
                    self.lock(ctx, round, propose_hash);
                }
            }
        }
    }

    fn handle_precommit(&self, ctx: &mut NodeContext, msg: Precommit) {
        // Add precommit
        let has_consensus = ctx.state.add_precommit(&msg);

        if has_consensus {
            let propose_hash = msg.propose_hash();
            // FIXME: у нас есть все транзакции
            if ctx.state.proposal(&propose_hash).is_none() {
                // Execute block and get state hash
                let block_hash = self.execute(ctx, propose_hash);

                if &block_hash != msg.block_hash() {
                    panic!("We are fucked up...");
                }

                self.commit(ctx, msg.round(), propose_hash);
            }
        }
    }

    fn commit(&self, ctx: &mut NodeContext,
              round: Round, hash: &Hash) {
        // Merge changes into storage
        // FIXME: remove unwrap here
        ctx.storage.merge(ctx.state.patch(hash).unwrap());

        // Update state to new height
        ctx.state.new_height(hash);

        // Generate new transactions
        for tx in (&mut ctx.tx_generator).take(100) {
            ctx.state.add_transaction(tx.hash(), tx);
        }

        // Send commit
        let height = ctx.state.height() - 1;
        self.send_commit(ctx, height, round, hash);

        // Handle queued messages
        for msg in ctx.state.queued() {
            self.handle(ctx, msg);
        }

        // Send propose
        if self.is_leader(ctx) {
            self.send_propose(ctx);
        }

        // Add timeout for first round
        ctx.add_timeout();
    }

    fn handle_commit(&self, _: &mut NodeContext, _: Commit) {
    }

    fn handle_timeout(&self, ctx: &mut NodeContext, timeout: Timeout) {
        if timeout.height != ctx.state.height() {
            return
        }

        if timeout.round != ctx.state.round() {
            return
        }

        // Add timeout for this round
        ctx.add_timeout();

        // Update state to new round
        ctx.state.new_round();

        // Send prevote if we are locked or propose if we are leader
        if let Some(hash) = ctx.state.locked_propose() {
            let round = ctx.state.round();
            self.send_prevote(ctx, round, &hash);
        } else if self.is_leader(ctx) {
            self.send_propose(ctx);
        }

        // Handle queued messages
        for msg in ctx.state.queued() {
            self.handle(ctx, msg);
        }
    }

    fn is_leader(&self, ctx: &NodeContext) -> bool {
        ctx.state.leader(ctx.state.round()) == ctx.state.id()
    }

    // FIXME: fix this bull shit
    fn execute(&self, ctx: &mut NodeContext, hash: &Hash) -> Hash {
        let fork = Fork::new(ctx.storage.as_ref());

        // fork.put_block(msg);

        ctx.state.set_patch(hash.clone(), fork.patch()).block_hash().clone()
    }

    fn send_propose(&self, ctx: &mut NodeContext) {
        let round = ctx.state.round();
        let txs : Vec<Hash> = ctx.state.transactions()
                                       .keys()
                                       .map(|h| h.clone())
                                       .collect();
        let propose = Propose::new(ctx.state.id(),
                                   ctx.state.height(),
                                   round,
                                   get_time(),
                                   &ctx.storage.prev_hash(),
                                   &txs,
                                   &ctx.secret_key);
        ctx.broadcast(propose.raw());

        let hash = propose.hash();
        ctx.state.add_propose(hash, &propose);

        // Send prevote
        self.send_prevote(ctx, round, &hash);
    }

    fn send_prevote(&self, ctx: &mut NodeContext,
                    round: Round, propose_hash: &Hash) {
        let locked_round = ctx.state.locked_round();
        if locked_round > 0 {
            debug_assert_eq!(&ctx.state.locked_propose().unwrap(), propose_hash);
        }
        let prevote = Prevote::new(ctx.state.id(),
                                   ctx.state.height(),
                                   round,
                                   propose_hash,
                                   locked_round,
                                   &ctx.secret_key);
        ctx.state.add_prevote(&prevote);
        ctx.broadcast(prevote.raw());
    }

    fn send_precommit(&self, ctx: &mut NodeContext,
                      round: Round, propose_hash: &Hash, block_hash: &Hash) {
        let precommit = Precommit::new(ctx.state.id(),
                                       ctx.state.height(),
                                       round,
                                       propose_hash,
                                       block_hash,
                                       &ctx.secret_key);
        ctx.state.add_precommit(&precommit);
        ctx.broadcast(precommit.raw());
    }

    fn send_commit(&self, ctx: &mut NodeContext,
                   height: Height, round: Round, propose_hash: &Hash) {
        // Send commit
        let commit = Commit::new(ctx.state.id(),
                                 height,
                                 round,
                                 propose_hash,
                                 &ctx.secret_key);
        ctx.broadcast(commit.raw());
    }
}

impl ConsensusHandler for ConsensusService {
    // default implementation
}
