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

use exonum::blockchain::Schema;
use log::{info, trace, warn};

use std::mem;

use super::{ConnectListConfig, ExternalMessage, NodeHandler, NodeTimeout};
use crate::events::{
    Event, EventHandler, EventOutcome, InternalEvent, InternalEventInner, InternalRequest,
    NetworkEvent,
};

impl EventHandler for NodeHandler {
    fn handle_event(&mut self, event: Event) -> EventOutcome {
        match event {
            Event::Network(network) => {
                self.handle_network_event(network);
                EventOutcome::Ok
            }
            Event::Transaction(tx) => {
                self.handle_incoming_tx(tx);
                EventOutcome::Ok
            }
            Event::Internal(internal) => {
                self.handle_internal_event(internal);
                EventOutcome::Ok
            }

            Event::Api(api) => self.handle_api_event(api),
        }
    }
}

impl NodeHandler {
    fn handle_internal_event(&mut self, event: InternalEvent) {
        match event.0 {
            InternalEventInner::Timeout(timeout) => self.handle_timeout(timeout),
            InternalEventInner::JumpToRound(height, round) => self.handle_new_round(height, round),
            InternalEventInner::MessageVerified(msg) => self.handle_message(*msg),
        }
    }

    fn handle_network_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::PeerConnected(peer, connect) => self.handle_connected(&peer, connect),
            NetworkEvent::PeerDisconnected(peer) => self.handle_disconnected(peer),
            NetworkEvent::UnableConnectToPeer(peer) => self.handle_unable_to_connect(peer),
            NetworkEvent::MessageReceived(raw) => {
                self.execute_later(InternalRequest::VerifyMessage(raw))
            }
        }
    }

    fn handle_api_event(&mut self, event: ExternalMessage) -> EventOutcome {
        match event {
            ExternalMessage::PeerAdd(info) => {
                info!("Send Connect message to {}", info);
                self.state.add_peer_to_connect_list(info.clone());
                self.connect(info.public_key);

                if let Some(ref mut config_manager) = self.config_manager {
                    let connect_list_config =
                        ConnectListConfig::from_connect_list(&self.state.connect_list());

                    config_manager.store_connect_list(connect_list_config);
                }
            }

            ExternalMessage::Enable(value) => {
                let s = if value { "enabled" } else { "disabled" };
                if self.is_enabled == value {
                    info!("Node is already {}", s);
                } else {
                    self.is_enabled = value;
                    self.api_state().set_enabled(value);
                    info!("The node is {} now", s);
                    if self.is_enabled {
                        self.request_next_block();
                    }
                }
            }

            ExternalMessage::Shutdown => {
                self.handle_shutdown();
                return EventOutcome::Terminated;
            }
        }

        EventOutcome::Ok
    }

    fn handle_timeout(&mut self, timeout: NodeTimeout) {
        match timeout {
            NodeTimeout::Round(epoch, round) => self.handle_round_timeout(epoch, round),
            NodeTimeout::Request(data, peer) => self.handle_request_timeout(&data, peer),
            NodeTimeout::Status(epoch) => self.handle_status_timeout(epoch),
            NodeTimeout::PeerExchange => self.handle_peer_exchange_timeout(),
            NodeTimeout::UpdateApiState => self.handle_update_api_state_timeout(),
            NodeTimeout::Propose(epoch, round) => self.handle_propose_timeout(epoch, round),
            NodeTimeout::FlushPool => {
                self.flush_txs_into_pool();
                self.maybe_add_flush_pool_timeout();
            }
        }
    }

    /// Schedule execution for later time.
    pub(crate) fn execute_later(&mut self, event: InternalRequest) {
        self.channel.internal_requests.send(event);
    }

    /// Shutdown current node.
    pub(crate) fn handle_shutdown(&mut self) {
        log::info!("Shutting down node handler");

        // Flush transactions stored in tx_cache to persistent pool.
        self.flush_txs_into_pool();
    }

    pub(crate) fn flush_txs_into_pool(&mut self) {
        let tx_cache_size = self.state().tx_cache_len();
        if tx_cache_size == 0 {
            return;
        } else {
            trace!(
                "Flushing {} transactions from cache to persistent pool",
                tx_cache_size
            );
        }

        let fork = self.blockchain.fork();
        let mut schema = Schema::new(&fork);
        for (_, tx) in mem::take(self.state.tx_cache_mut()) {
            schema.add_transaction_into_pool(tx);
        }

        if self.blockchain.merge(fork.into_patch()).is_err() {
            warn!("Failed to flush transactions from cache to persistent pool.");
        }
    }
}
