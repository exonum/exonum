extern crate rand;

use std::net::SocketAddr;

use rand::Rng;

use ::messages::{Any, RawMessage, Connect, Status, Message, RequestPeers};
use ::events::Channel;

use super::{NodeHandler, RequestData, ExternalMessage, NodeTimeout};

impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn handle_message(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }

        let msg = Any::from_raw(raw).unwrap();
        match msg {
            Any::Connect(msg) => self.handle_connect(msg),
            Any::Status(msg) => self.handle_status(msg),
            Any::Consensus(msg) => self.handle_consensus(msg),
            Any::Request(msg) => self.handle_request(msg),
            Any::Block(msg) => self.handle_block(msg),
            Any::Transaction(msg) => self.handle_tx(msg),
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
        // Handle message from future height
        if msg.height() > height {
            // Check validator height info
            // FIXME: make sure that validator id < validator count
            if msg.height() > self.state.validator_height(msg.validator()) {
                // Update validator height
                self.state.set_validator_height(msg.validator(), msg.height());
            }
            // Verify validator if and signature
            let peer = match self.state.public_key_of(msg.validator()) {
                // Incorrect signature of message
                Some(public_key) => {
                    if !msg.verify_signature(public_key) {
                        return;
                    }
                    *public_key
                }
                // Incorrect validator id
                None => return,
            };
            // Request block
            self.request(RequestData::Block(height), peer);
        }
    }

    pub fn handle_request_peers(&mut self, msg: RequestPeers) {
        let peers: Vec<Connect> = self.state.peers().iter().map(|(_, b)| b.clone()).collect();
        for peer in peers {
            self.send_to_peer(*msg.from(), peer.raw());
        }
    }

    pub fn handle_status_timeout(&mut self) {
        let hash = self.blockchain.last_hash().unwrap();
        // Send status
        let status = Status::new(self.state.id(),
                                 self.state.height(),
                                 &hash,
                                 self.state.secret_key());
        trace!("Broadcast status: {:?}", status);
        self.broadcast(status.raw());

        self.add_status_timeout();
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
            let msg = RequestPeers::new(self.state.public_key(),
                                        peer.pub_key(),
                                        self.state.secret_key());
            trace!("Request peers from peer with addr {:?}", peer.addr());
            self.send_to_peer(*peer.pub_key(), msg.raw());
        }
        self.add_peer_exchange_timeout();
    }
}
