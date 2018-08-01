// Copyright 2018 The Exonum Team
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

use super::{ConnectListConfig, ExternalMessage, NodeHandler, NodeTimeout};
use events::{error::LogError, Event, EventHandler, InternalEvent, InternalRequest, NetworkEvent};

impl EventHandler for NodeHandler {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Network(network) => self.handle_network_event(network),
            Event::Api(api) => self.handle_api_event(api),
            Event::Internal(internal) => self.handle_internal_event(internal),
        }
    }
}

impl NodeHandler {
    // clippy sure that `InternalEvent` is not consumed in the body
    // this is because of internal `Copy` types in `JumpToRound`.
    #![cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
    fn handle_internal_event(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::Timeout(timeout) => self.handle_timeout(timeout),
            InternalEvent::JumpToRound(height, round) => self.handle_new_round(height, round),
            InternalEvent::Shutdown => panic!("Shutdown should be processed in the event loop"),
        }
    }

    fn handle_network_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::PeerConnected(peer, connect) => self.handle_connected(&peer, connect),
            NetworkEvent::PeerDisconnected(peer) => self.handle_disconnected(peer),
            NetworkEvent::UnableConnectToPeer(peer) => self.handle_unable_to_connect(peer),
            NetworkEvent::MessageReceived(_, raw) => self.handle_message(raw),
        }
    }

    fn handle_api_event(&mut self, event: ExternalMessage) {
        match event {
            ExternalMessage::Transaction(tx) => {
                self.handle_incoming_tx(tx);
            }
            ExternalMessage::PeerAdd(info) => {
                info!("Send Connect message to {}", info);
                self.state.add_peer_to_connect_list(info);
                self.connect(&info.address);

                if self.config_manager.is_some() {
                    let connect_list_config =
                        ConnectListConfig::from_connect_list(&self.state.connect_list());

                    self.config_manager
                        .as_ref()
                        .unwrap()
                        .store_connect_list(connect_list_config);
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
                        self.add_round_timeout();
                    }
                }
            }
            ExternalMessage::Shutdown => self.execute_later(InternalRequest::Shutdown),
        }
    }

    fn handle_timeout(&mut self, timeout: NodeTimeout) {
        if !self.is_enabled {
            info!(
                "Ignoring a timeout {:?} because the node is disabled",
                timeout
            );
            return;
        }
        match timeout {
            NodeTimeout::Round(height, round) => self.handle_round_timeout(height, round),
            NodeTimeout::Request(data, peer) => self.handle_request_timeout(&data, peer),
            NodeTimeout::Status(height) => self.handle_status_timeout(height),
            NodeTimeout::PeerExchange => self.handle_peer_exchange_timeout(),
            NodeTimeout::UpdateApiState => self.handle_update_api_state_timeout(),
            NodeTimeout::Propose(height, round) => self.handle_propose_timeout(height, round),
        }
    }

    /// Schedule execution for later time
    pub(crate) fn execute_later(&mut self, event: InternalRequest) {
        self.channel.internal_requests.send(event).log_error();
    }
}
