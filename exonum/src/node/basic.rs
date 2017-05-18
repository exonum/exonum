extern crate rand;

use rand::Rng;

use std::net::SocketAddr;

use messages::{Any, RawMessage, Connect, Status, Message, RequestPeers};
use events::Channel;
use super::{NodeHandler, RequestData, ExternalMessage, NodeTimeout, Height};

impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn handle_message(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }
        
        match Any::from_raw(raw) {
            Ok(Any::Connect(msg)) => self.handle_connect(msg),
            Ok(Any::Status(msg)) => self.handle_status(msg),
            Ok(Any::Consensus(msg)) => self.handle_consensus(msg),
            Ok(Any::Request(msg)) => self.handle_request(msg),
            Ok(Any::Block(msg)) => self.handle_block(msg),
            Ok(Any::Transaction(msg)) => self.handle_tx(msg),
            Err(err) => {
                // TODO: Replace by `err.description()` after #103 is merged.
                error!("Invalid message received: {:?}", err);
            }
        }
    }

    pub fn handle_connected(&mut self, addr: &SocketAddr) {
        info!("Connected to: {}", addr);
        let message = self.state.our_connect_message().clone();
        self.send_to_addr(addr, message.raw());
    }

    pub fn handle_disconnected(&mut self, addr: &SocketAddr) {
        info!("Disconnected from: {}", addr);
        let need_reconnect = self.state.remove_peer_with_addr(addr);
        if need_reconnect {
            self.connect(addr);
        }
    }

    pub fn handle_connect(&mut self, message: Connect) {
        // TODO add spam protection
        let address = message.addr();
        if address == self.state.our_connect_message().addr() {
            return;
        }

        if !self.state.whitelist().allow(message.pub_key()) {
            error!("Received connect message from peer = {:?} which not in whitelist.", message.pub_key());
            return;
        }

        // Check if we have another connect message from peer with the given public_key
        let public_key = *message.pub_key();
        let mut need_connect = true;
        if let Some(saved_message) = self.state.peers().get(&public_key) {
            if saved_message.time() > message.time() {
                error!("Received outdated Connect message from {}", address);
                return;
            } else if saved_message.time() < message.time() {
                need_connect = saved_message.addr() != message.addr();
            } else if saved_message.addr() != message.addr() {
                error!("Received weird Connect message from {}", address);
                return;
            }
        }
        info!("Received Connect message from {}, {}",
              address,
              need_connect);
        self.state.add_peer(public_key, message);
        if need_connect {
            // TODO: reduce double sending of connect message
            info!("Send Connect message to {}", address);
            self.connect(&address);
        }
    }

    pub fn handle_status(&mut self, msg: Status) {
        let height = self.state.height();
        trace!("HANDLE STATUS: current height = {}, msg height = {}", height, msg.height());

        if !self.state.whitelist().allow(msg.from()) {
            error!("Received status message from peer = {:?} which not in whitelist.", msg.from());
            return;
        }

        // Handle message from future height
        if msg.height() > height {
            let peer = msg.from();

            //verify message
            if !msg.verify_signature(peer) {
                return;
            }
            
            // Check validator height info
            if msg.height() > self.state.node_height(peer) {
                // Update validator height
                self.state.set_node_height(*peer, msg.height());
            }

            // Request block
            self.request(RequestData::Block(height), *peer);
        }
    }

    pub fn handle_request_peers(&mut self, msg: RequestPeers) {
        let peers: Vec<Connect> = self.state.peers().iter().map(|(_, b)| b.clone()).collect();
        trace!("HANDLE REQUEST PEERS: Sending {:?} peers to {:?}", peers, msg.from());

        for peer in peers {
            self.send_to_peer(*msg.from(), peer.raw());
        }
    }

    pub fn handle_status_timeout(&mut self, height: Height) {
        if self.state.height() == height {
            self.broadcast_status();
            self.add_status_timeout();
        }
    }

    pub fn handle_peer_exchange_timeout(&mut self) {
        if !self.state.peers().is_empty() {
            let to = self.state.peers().len();
            let gen_peer_id = || -> usize {
                let mut rng = rand::thread_rng();
                rng.gen_range(0, to)
            };

            let peer = self.state
                .peers()
                .iter()
                .map(|x| x.1.clone())
                .nth(gen_peer_id())
                .unwrap();
            let peer = peer.clone();
            let msg = RequestPeers::new(self.state.consensus_public_key(),
                                        peer.pub_key(),
                                        self.state.consensus_secret_key());
            trace!("Request peers from peer with addr {:?}", peer.addr());
            self.send_to_peer(*peer.pub_key(), msg.raw());
        }
        self.add_peer_exchange_timeout();
    }

    pub fn broadcast_status(&mut self) {
        let hash = self.blockchain.last_hash().unwrap();
        let status = Status::new(self.state.consensus_public_key(),
                                 self.state.height(),
                                 &hash,
                                 self.state.consensus_secret_key());
        trace!("Broadcast status: {:?}", status);
        self.broadcast(status.raw());
    }
}
