// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Testing cryptocurrency together with a toy multisig service. The multisig service does not allow
//! changing membership, cancelling proposals, etc.

use exonum::{
    crypto::{Hash, KeyPair, PublicKey},
    merkledb::{
        access::{Access, FromAccess},
        BinaryValue, Entry, MapIndex, ObjectHash,
    },
    runtime::{AnyTx, Caller, CallerAddress, CommonError, ExecutionError, SnapshotExt},
};
use exonum_derive::*;
use exonum_rust_runtime::{
    ExecutionContext, GenericCallMut, MethodDescriptor, Service, ServiceFactory, TxStub,
};
use exonum_testkit::{TestKit, TestKitBuilder};
use serde_derive::{Deserialize, Serialize};

use exonum_cryptocurrency_advanced::{
    transactions::{CreateWallet, Transfer},
    CryptocurrencyInterface, CryptocurrencyService, Schema,
};

/// Service instance ID.
const SERVICE_ID: u32 = 120;
/// Multisig service ID.
const MULTISIG_ID: u32 = SERVICE_ID + 1;

#[derive(Debug, Serialize, Deserialize, BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Config {
    participants: Vec<CallerAddress>,
    threshold: u32,
}

impl Config {
    fn new(participants: Vec<PublicKey>, threshold: usize) -> Self {
        assert!(threshold > 1 && threshold <= participants.len());
        Self {
            participants: participants
                .into_iter()
                .map(|pk| Caller::Transaction { author: pk }.address())
                .collect(),
            threshold: threshold as u32,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
struct Proposal {
    action: AnyTx,
    votes: Vec<bool>,
}

#[derive(FromAccess)]
struct MultisigSchema<T: Access> {
    config: Entry<T::Base, Config>,
    proposals: MapIndex<T::Base, Hash, Proposal>,
}

impl<T: Access> MultisigSchema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("MultisigInterface"))]
#[service_factory(artifact_name = "toy-multisig")]
struct MultisigService;

impl Service for MultisigService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let config = Config::from_bytes(params.into()).map_err(CommonError::malformed_arguments)?;
        let mut schema = MultisigSchema::new(context.service_data());
        schema.config.set(config);
        Ok(())
    }
}

#[exonum_interface]
trait MultisigInterface<Ctx> {
    type Output;

    /// Proposes an `action` to be authorized by the service.
    ///
    /// This call should be authorized by one of multisig participants.
    #[interface_method(id = 0)]
    fn propose_action(&self, context: Ctx, action: AnyTx) -> Self::Output;

    /// Approves an earlier proposed action. If the action has reached threshold approval,
    /// it is executed with the service authorization and removed from the pending actions.
    ///
    /// This call should be authorized by one of multisig participants.
    #[interface_method(id = 1)]
    fn support_action(&self, context: Ctx, action_hash: Hash) -> Self::Output;
}

impl MultisigInterface<ExecutionContext<'_>> for MultisigService {
    type Output = Result<(), ExecutionError>;

    fn propose_action(&self, context: ExecutionContext<'_>, action: AnyTx) -> Self::Output {
        let caller = context.caller().address();
        // Note that identifying a proposal by the enclosing transaction hash is not always sound.
        // Indeed, multiple proposals may be created in the same transaction, e.g., if
        // batching is used. Also, getting transaction hash will fail if a call is made
        // from a service hook.
        let tx_hash = context
            .transaction_hash()
            .ok_or(CommonError::UnauthorizedCaller)?;

        let mut schema = MultisigSchema::new(context.service_data());
        let config = schema.config.get().unwrap();
        let caller_index = config
            .participants
            .iter()
            .position(|addr| *addr == caller)
            .ok_or(CommonError::UnauthorizedCaller)?;

        let mut votes = vec![false; config.participants.len()];
        votes[caller_index] = true;
        let proposal = Proposal { action, votes };
        schema.proposals.put(&tx_hash, proposal);
        Ok(())
    }

    fn support_action(&self, mut context: ExecutionContext<'_>, action_hash: Hash) -> Self::Output {
        let caller = context.caller().address();

        let (config, mut proposal) = {
            let schema = MultisigSchema::new(context.service_data());
            let config = schema.config.get().unwrap();
            let proposal = schema
                .proposals
                .get(&action_hash)
                .ok_or_else(|| ExecutionError::service(0, "Proposal not found"))?;
            (config, proposal)
        };

        let caller_index = config
            .participants
            .iter()
            .position(|addr| *addr == caller)
            .ok_or(CommonError::UnauthorizedCaller)?;
        proposal.votes[caller_index] = true;

        let current_votes: u32 = proposal.votes.iter().map(|flag| *flag as u32).sum();
        if current_votes == config.threshold {
            let call_info = proposal.action.call_info;
            let method = MethodDescriptor::inherent(call_info.method_id);
            context.generic_call_mut(call_info.instance_id, method, proposal.action.arguments)?;
            MultisigSchema::new(context.service_data())
                .proposals
                .remove(&action_hash);
        } else {
            MultisigSchema::new(context.service_data())
                .proposals
                .put(&action_hash, proposal);
        }

        Ok(())
    }
}

fn create_testkit_with_multisig(keys: Vec<PublicKey>, threshold: usize) -> TestKit {
    let artifact = CryptocurrencyService.artifact_id();
    let ms_artifact = MultisigService.artifact_id();
    let ms_instance = ms_artifact
        .clone()
        .into_default_instance(MULTISIG_ID, "multisig")
        .with_constructor(Config::new(keys, threshold));

    TestKitBuilder::validator()
        .with_rust_service(CryptocurrencyService)
        .with_rust_service(MultisigService)
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(SERVICE_ID, "token"))
        .with_artifact(ms_artifact)
        .with_instance(ms_instance)
        .build()
}

#[test]
fn test_multisig() {
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let mut testkit = create_testkit_with_multisig(vec![alice.public_key(), bob.public_key()], 2);
    let ms_address = Caller::Service {
        instance_id: MULTISIG_ID,
    }
    .address();

    // Create the multisig wallet. This takes two separate transactions from both
    // multisig participants.
    let action = TxStub.create_wallet(SERVICE_ID, CreateWallet::new("Alice + Bob"));
    let tx = alice.propose_action(MULTISIG_ID, action);
    let action_hash = tx.object_hash();
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();
    let tx = bob.support_action(MULTISIG_ID, action_hash);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();

    // Check that the multisig wallet is created.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let ms_wallet = schema.wallets.get(&ms_address).unwrap();
    assert_eq!(ms_wallet.name, "Alice + Bob");
    assert_eq!(ms_wallet.balance, 100);

    // Spend some tokens from the wallet!
    let alice_address = CallerAddress::from_key(alice.public_key());
    let action = TxStub.transfer(
        SERVICE_ID,
        Transfer {
            to: alice_address,
            amount: 15,
            seed: 0,
        },
    );

    let tx = bob.propose_action(MULTISIG_ID, action);
    let action_hash = tx.object_hash();
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();
    // Since approvals from both Alice and Bob are necessary, the balance should remain the same
    // for now.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let ms_wallet = schema.wallets.get(&ms_address).unwrap();
    assert_eq!(ms_wallet.balance, 100);

    let tx = alice.create_wallet(SERVICE_ID, CreateWallet::new("Alice"));
    testkit.create_block_with_transaction(tx);

    let tx = alice.support_action(MULTISIG_ID, action_hash);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();
    // Now the balance of the multisig service should change.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let ms_wallet = schema.wallets.get(&ms_address).unwrap();
    assert_eq!(ms_wallet.balance, 85);
    let alice_wallet = schema.wallets.get(&alice_address).unwrap();
    assert_eq!(alice_wallet.balance, 115);
}

#[test]
fn test_2_of_3_multisig() {
    let alice = KeyPair::random();
    let bob = KeyPair::random();
    let carol = KeyPair::random();

    let mut testkit = create_testkit_with_multisig(
        vec![alice.public_key(), bob.public_key(), carol.public_key()],
        2,
    );
    let ms_address = Caller::Service {
        instance_id: MULTISIG_ID,
    }
    .address();

    let action = TxStub.create_wallet(SERVICE_ID, CreateWallet::new("Alice + Bob"));
    let alice_tx = alice.propose_action(MULTISIG_ID, action);
    let action_hash = alice_tx.object_hash();
    let bob_tx = bob.support_action(MULTISIG_ID, action_hash);
    let block = testkit.create_block_with_transactions(vec![alice_tx, bob_tx]);
    block[0].status().unwrap();
    block[1].status().unwrap();

    // Check that the wallet has been created.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let ms_wallet = schema.wallets.get(&ms_address).unwrap();
    assert_eq!(ms_wallet.balance, 100);

    let action = TxStub.transfer(
        SERVICE_ID,
        Transfer {
            to: CallerAddress::from_key(carol.public_key()),
            amount: 10,
            seed: 0,
        },
    );
    let create_wallet_tx = carol.create_wallet(SERVICE_ID, CreateWallet::new("Carol"));
    let alice_tx = alice.propose_action(MULTISIG_ID, action);
    let action_hash = alice_tx.object_hash();
    let carol_tx = carol.support_action(MULTISIG_ID, action_hash);
    let block = testkit.create_block_with_transactions(vec![create_wallet_tx, alice_tx, carol_tx]);
    block[0].status().unwrap();
    block[1].status().unwrap();

    // Check the change in the multisig wallet balance.
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(SERVICE_ID).unwrap();
    let ms_wallet = schema.wallets.get(&ms_address).unwrap();
    assert_eq!(ms_wallet.balance, 90);
}
