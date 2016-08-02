use std::collections::HashSet;

use super::super::crypto::{Hash, hash};
use super::super::messages::{
    ConsensusMessage, Propose, Prevote, Precommit, Commit, Message,
    RequestPropose, RequestTransactions, RequestPrevotes,
    RequestPrecommits, RequestCommit,
    RequestPeers, TxMessage
};
use super::super::storage::{Map};
use super::{NodeContext, Round, Height, RequestData, ValidatorId};

pub struct ConsensusService;

pub trait ConsensusHandler {
    fn handle(&self, ctx: &mut NodeContext, msg: ConsensusMessage) {
        info!("handle consensus message");
        // Ignore messages from previous and future height
        if msg.height() < ctx.state.height() || msg.height() > ctx.state.height() + 1 {
            return
        }

        if let ConsensusMessage::Commit(msg) = msg {
            self.handle_commit(ctx, msg);
            return
        }

        // Queued messages from next height or round
        if msg.height() == ctx.state.height() + 1 ||
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
                if msg.prev_hash() != &ctx.storage.last_hash().unwrap().unwrap_or_else(|| hash(&[])) {
                    return
                }

                // Check leader
                if msg.validator() != ctx.state.leader(msg.round()) {
                    return
                }

                self.handle_propose(ctx, msg)
            },
            ConsensusMessage::Prevote(msg) => self.handle_prevote(ctx, msg),
            ConsensusMessage::Precommit(msg) => self.handle_precommit(ctx, msg),
            ConsensusMessage::Commit(msg) => self.handle_commit(ctx, msg),
        }
    }

    fn handle_propose(&self, ctx: &mut NodeContext, propose: Propose) {
        info!("recv propose");
        // TODO: check time
        // TODO: check that transactions are not commited yet

        // Add propose
        let hash = propose.hash();
        let added = ctx.state.add_propose(hash, &propose);

        if added {
            // Remove request info
            let known_nodes = self.remove_request(ctx, RequestData::Propose(hash));

            if ctx.state.propose(&hash).unwrap().has_unknown_txs() {
                self.request(ctx, RequestData::Transactions(hash), propose.validator());
                for node in known_nodes {
                    self.request(ctx, RequestData::Transactions(hash), node);
                }
            } else {
                self.has_full_propose(ctx, hash);
            }
        }
    }

    fn has_full_propose(&self, ctx: &mut NodeContext, hash: Hash) {
        // Remove request info
        self.remove_request(ctx, RequestData::Transactions(hash.clone()));

        let propose_round = ctx.state.propose(&hash).unwrap().message().round();

        // Send prevote
        if ctx.state.locked_round() == 0 {
            self.send_prevote(ctx, propose_round, &hash);
        }

        // Lock to propose
        // TODO: avoid loop here
        let start_round = ::std::cmp::max(ctx.state.locked_round() + 1,
                                          propose_round);
        for round in start_round ... ctx.state.round() {
            if ctx.state.has_majority_prevotes(round, hash) {
                self.has_majority_prevotes(ctx, round, &hash);
            }
        }

        // Commit propose
        for (round, block_hash) in ctx.state.unknown_propose_with_precommits(&hash) {
            // Execute block and get state hash
            let our_block_hash = self.execute(ctx, &hash);

            if our_block_hash != block_hash {
                panic!("We are fucked up...");
            }

            self.commit(ctx, round, &hash);
        }
    }

    fn handle_prevote(&self, ctx: &mut NodeContext, prevote: Prevote) {
        info!("recv prevote");
        // Add prevote
        let has_consensus = ctx.state.add_prevote(&prevote);

        // Request propose or transactions
        self.request_propose_or_txs(ctx,
                                    prevote.propose_hash(),
                                    prevote.validator());

        // Request prevotes
        if prevote.locked_round() > ctx.state.locked_round() {
            self.request(ctx,
                         RequestData::Prevotes(prevote.locked_round(),
                                               *prevote.propose_hash()),
                         prevote.validator());
        }

        // Lock to propose
        if has_consensus {
            self.has_majority_prevotes(ctx,
                                       prevote.round(),
                                       prevote.propose_hash());
        }
    }

    fn has_majority_prevotes(&self, ctx: &mut NodeContext,
                             round: Round, propose_hash: &Hash) {
        // Remove request info
        self.remove_request(ctx, RequestData::Prevotes(round, *propose_hash));
        // Lock to propose
        if ctx.state.locked_round() < round {
            // FIXME: проверка что у нас есть все транзакции
            if ctx.state.propose(propose_hash).is_some() {
                self.lock(ctx, round, *propose_hash);
            }
        }
    }

    fn has_majority_precommits(&self, ctx: &mut NodeContext,
                               round: Round,
                               propose_hash: &Hash,
                               block_hash: &Hash) {
        // Remove request info
        self.remove_request(ctx, RequestData::Precommits(round, *propose_hash, *block_hash));
        // Commit
        if ctx.state.propose(&propose_hash).is_some() {
            // FIXME: проверка что у нас есть все транзакции

            // Execute block and get state hash
            let our_block_hash = self.execute(ctx, propose_hash);

            if &our_block_hash != block_hash {
                panic!("We are fucked up...");
            }

            self.commit(ctx, round, propose_hash);
        } else {
            ctx.state.add_unknown_propose_with_precommits(round,
                                                          *propose_hash,
                                                          *block_hash);
        }
    }

    fn lock(&self, ctx: &mut NodeContext,
            round: Round, propose_hash: Hash) {
        info!("MAKE LOCK");
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
                self.has_majority_precommits(ctx, round, &propose_hash, &block_hash);
                return
            }
        }

        // Send prevotes
        for round in ctx.state.locked_round() + 1 ... ctx.state.round() {
            if !ctx.state.have_prevote(round) {
                self.send_prevote(ctx, round, &propose_hash);
                if ctx.state.has_majority_prevotes(round, propose_hash) {
                    self.has_majority_prevotes(ctx, round, &propose_hash);
                }
            }
        }
    }

    fn handle_precommit(&self, ctx: &mut NodeContext, msg: Precommit) {
        info!("recv precommit");
        // Add precommit
        let has_consensus = ctx.state.add_precommit(&msg);

        // Request propose
        if let None = ctx.state.propose(msg.propose_hash()) {
            self.request(ctx,
                         RequestData::Propose(*msg.propose_hash()),
                         msg.validator());
        }

        // Request prevotes
        // FIXME: если отправитель precommit находится на бОльшей высоте,
        // у него уже нет +2/3 prevote. Можем ли мы избавится от бесполезной
        // отправки RequestPrevotes?
        if msg.round() > ctx.state.locked_round() {
            self.request(ctx,
                         RequestData::Prevotes(msg.round(),
                                               *msg.propose_hash()),
                         msg.validator());
        }

        // Has majority precommits
        if has_consensus {
            self.has_majority_precommits(ctx,
                                         msg.round(),
                                         msg.propose_hash(),
                                         msg.block_hash());
        }
    }

    fn commit(&self, ctx: &mut NodeContext,
              round: Round, hash: &Hash) {
        info!("COMMIT");
        // Merge changes into storage
        // FIXME: remove unwrap here, merge patch into storage
        ctx.storage.merge(ctx.state.propose(hash).unwrap().patch().unwrap()).is_ok();

        // FIXME: use block hash here
        let block_hash = hash;

        // Update state to new height
        ctx.state.new_height(hash);

        // Generate new transactions
        let txs = (&mut ctx.tx_generator).take(100).collect(): Vec<_>;
        for tx in txs {
            ctx.broadcast(&tx.raw().clone());
            ctx.state.add_transaction(tx.hash(), tx);
        }

        // Send commit
        let height = ctx.state.height() - 1;
        self.send_commit(ctx, height, round, hash, &block_hash);

        // Handle queued messages
        for msg in ctx.state.queued() {
            self.handle(ctx, msg);
        }

        // Send propose
        if self.is_leader(ctx) {
            self.send_propose(ctx);
        }

        // Add timeout for first round
        ctx.add_round_timeout();

        // Request commits
        for validator in ctx.state.validator_heights() {
            self.request(ctx, RequestData::Commit, validator)
        }
    }

    fn handle_tx(&mut self, ctx: &mut NodeContext, msg: TxMessage) {
        info!("recv tx");
        let hash = msg.hash();

        // Make sure that it is new transaction
        // TODO: use contains instead of get?
        if ctx.storage.transactions().get(&hash).unwrap().is_some() {
            return;
        }

        if ctx.state.transactions().contains_key(&hash) {
            return;
        }


        let full_proposes = ctx.state.add_transaction(hash, msg);

        // Go to has full propose if we get last transaction
        for hash in full_proposes {
            self.has_full_propose(ctx, hash);
        }
    }

    fn handle_commit(&self, ctx: &mut NodeContext, msg: Commit) {
        info!("recv commit");
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
            self.request(ctx, RequestData::Commit, msg.validator());

            return;
        }

        // Handle message from current height
        if msg.height() == ctx.state.height() {
            // Request propose or txs
            self.request_propose_or_txs(ctx, msg.propose_hash(), msg.validator());

            // Request precommits
            if !ctx.state.has_majority_precommits(msg.round(),
                                                  *msg.propose_hash(),
                                                  *msg.block_hash()) {
                let data = RequestData::Precommits(msg.round(),
                                                  *msg.propose_hash(),
                                                  *msg.block_hash());
                self.request(ctx, data, msg.validator());
            }
        }
    }

    fn handle_round_timeout(&self, ctx: &mut NodeContext,
                            height: Height, round: Round) {
        info!("ROUND TIMEOUT height={}, round={}", height, round);
        if height != ctx.state.height() {
            return
        }

        if round != ctx.state.round() {
            return
        }

        // Update state to new round
        ctx.state.new_round();

        // Add timeout for this round
        ctx.add_round_timeout();

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

    fn handle_request_timeout(&self, ctx: &mut NodeContext,
                              data: RequestData, validator: ValidatorId) {
        info!("REQUEST TIMEOUT");
        if let Some(validator) = ctx.state.retry(&data, validator) {
            ctx.add_request_timeout(data.clone(), validator);

            let message = match &data {
                &RequestData::Propose(ref propose_hash) =>
                    RequestPropose::new(ctx.state.id(),
                                        validator,
                                        ctx.events.get_time(),
                                        ctx.state.height(),
                                        propose_hash,
                                        &ctx.secret_key).raw().clone(),
                &RequestData::Transactions(ref propose_hash) => {
                    let txs : Vec<_> = ctx.state.propose(propose_hash)
                                                .unwrap()
                                                .unknown_txs()
                                                .iter()
                                                .map(|tx| *tx)
                                                .collect();
                    RequestTransactions::new(ctx.state.id(),
                                             validator,
                                             ctx.events.get_time(),
                                             &txs,
                                             &ctx.secret_key).raw().clone()
                },
                &RequestData::Prevotes(round, ref propose_hash) =>
                    RequestPrevotes::new(ctx.state.id(),
                                         validator,
                                         ctx.events.get_time(),
                                         ctx.state.height(),
                                         round,
                                         propose_hash,
                                         &ctx.secret_key).raw().clone(),
                &RequestData::Precommits(round, ref propose_hash, ref block_hash) =>
                    RequestPrecommits::new(ctx.state.id(),
                                        validator,
                                        ctx.events.get_time(),
                                        ctx.state.height(),
                                        round,
                                        propose_hash,
                                        block_hash,
                                        &ctx.secret_key).raw().clone(),
                &RequestData::Commit =>
                    RequestCommit::new(ctx.state.id(),
                                       validator,
                                       ctx.events.get_time(),
                                       ctx.state.height(),
                                       &ctx.secret_key).raw().clone(),
                &RequestData::Peers =>
                    RequestPeers::new(ctx.state.id(),
                                      validator,
                                      ctx.events.get_time(),
                                      &ctx.secret_key).raw().clone()
            };
            ctx.send_to_validator(validator, &message);
        }
    }

    fn is_leader(&self, ctx: &NodeContext) -> bool {
        ctx.state.leader(ctx.state.round()) == ctx.state.id()
    }

    // FIXME: fix this bull shit
    fn execute(&self, ctx: &mut NodeContext, hash: &Hash) -> Hash {
        let mut fork = ctx.storage.fork();

        let msg = ctx.state.propose(hash).unwrap().message().clone();

        // Update height
        fork.heights().append(*hash).is_ok();
        // Save propose
        fork.proposes().put(hash, msg.clone()).is_ok();
        // Save transactions
        for hash in msg.transactions() {
            fork.transactions().put(hash, ctx.state.transactions().get(hash).unwrap().clone()).is_ok();
        }
        // FIXME: put precommits

        // Save patch
        ctx.state.propose(hash).unwrap().set_patch(fork.patch());

        hash.clone()
    }

    fn request_propose_or_txs(&self, ctx: &mut NodeContext,
                              propose_hash: &Hash, validator: ValidatorId) {
        let requested_data = match ctx.state.propose(propose_hash) {
            Some(state) => {
                // Request transactions
                if state.has_unknown_txs() {
                    Some(RequestData::Transactions(*propose_hash))
                } else {
                    None
                }
            },
            None => {
                // Request propose
                Some(RequestData::Propose(*propose_hash))
            }
        };

        if let Some(data) = requested_data {
            self.request(ctx, data, validator);
        }
    }

    fn request(&self, ctx: &mut NodeContext,
               data: RequestData, validator: ValidatorId) {
        info!("REQUEST");
        let is_new = ctx.state.request(data.clone(), validator);

        if is_new {
            ctx.add_request_timeout(data, validator);
        }
    }

    fn remove_request(&self, ctx: &mut NodeContext, data: RequestData) -> HashSet<ValidatorId> {
        // TODO: clear timeout
        ctx.state.remove_request(&data)
    }

    fn send_propose(&self, ctx: &mut NodeContext) {
        info!("send propose");
        let round = ctx.state.round();
        let txs : Vec<Hash> = ctx.state.transactions()
                                       .keys()
                                       .map(|h| h.clone())
                                       .collect();
        let propose = Propose::new(ctx.state.id(),
                                   ctx.state.height(),
                                   round,
                                   ctx.events.get_time(),
                                   &ctx.storage.last_hash().unwrap().unwrap_or_else(|| hash(&[])),
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
        info!("send prevote");
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
        info!("send precommit");
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
                   height: Height, round: Round,
                   propose_hash: &Hash, block_hash: &Hash) {
        info!("send commit");
        // Send commit
        let commit = Commit::new(ctx.state.id(),
                                 height,
                                 round,
                                 propose_hash,
                                 block_hash,
                                 &ctx.secret_key);
        ctx.broadcast(commit.raw());
    }
}

impl ConsensusHandler for ConsensusService {
    // default implementation
}
