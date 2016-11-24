use node::serde_json;
use events::Channel;
use node::{ExternalMessage, NodeHandler, NodeTimeout};
use super::super::messages::{ConfigPropose, ConfigVote};
use super::super::blockchain::{Blockchain, View};
use super::super::crypto::{PublicKey, Hash};
use super::super::storage::Map;
use super::super::messages::Message;
use byteorder::{ByteOrder, LittleEndian};

#[derive(Debug, Serialize, Deserialize)]
pub struct Configuration {
    actual_from: u64,
    validators: Vec<PublicKey>,
    consensus: ConsensusCfg
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsensusCfg {
    round_timeout: u64,    // 2000
    status_timeout: u64,   // 5000
    peers_timeout: u64,    // 10000
    propose_timeout: u64,  // 500
    txs_block_limit: u16   // 500
}

trait ConfigurationValidator {
    fn is_valid(&self) -> bool;
}

impl ConfigurationValidator for ConsensusCfg {
    fn is_valid(&self) -> bool {
        self.round_timeout < 10000
    }
}

impl Configuration {

    #[allow(dead_code)]
    fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    #[allow(dead_code)]
    fn deserialize(serialized: &[u8]) -> Result<Configuration, &str> {
        let cfg: Configuration = serde_json::from_slice(serialized).unwrap();
        if cfg.is_valid() {
            return Ok(cfg);
        }
        Err("not valid")
    }
}

impl ConfigurationValidator for Configuration {
    fn is_valid(&self) -> bool {
        self.consensus.is_valid()
    }
}


impl<B, S> NodeHandler<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout> + Clone
{
    pub fn handle_config_propose(&self, config_propose: ConfigPropose) {

        if config_propose.height() < self.state.height() || config_propose.height() > self.state.height() + 1 {
            warn!("Received ConfigPropose message from other height: msg.height={}, self.height={}",
                  config_propose.height(),
                  self.state.height());
            return;
        }

        if config_propose.actual_from_height() < self.state.height() {
            error!("Received config for past height: msg.actual_from_height={}, self.height={}",
                config_propose.actual_from_height(), self.state.height());
            return;
        }

        if !self.state.validators().contains(config_propose.from()){
            error!("ConfigPropose from unknown validator: {:?}", config_propose.from());
            return;
        }

        let view = self.blockchain.view();
        if view.config_proposes().get(&config_propose.hash()).unwrap().is_some() {
            error!("Received config_propose has already been handled, msg={:?}", config_propose);
            return;
        }

        trace!("Handle ConfigPropose");
        let _ = view.config_proposes().put(&config_propose.hash(), config_propose);

    }

    pub fn handle_config_vote(&self, config_vote: ConfigVote){

        if config_vote.height() < self.state.height() || config_vote.height() > self.state.height() + 1 {
            warn!("Received ConfigVote message from other height: msg.height={}, self.height={}",
                  config_vote.height(),
                  self.state.height());
            return;
        }

        if !self.state.validators().contains(config_vote.from()){
            error!("ConfigVote from unknown validator: {:?}", config_vote.from());
            return;
        }

        let view = self.blockchain.view();
        if view.config_proposes().get(&config_vote.hash_propose()).unwrap().is_some() {
            error!("Received config_vote for unknown transaciton, msg={:?}", config_vote);
            return;
        }

        if let Some(vote) = view.config_votes().get(&config_vote.from()).unwrap() {
            if vote.seed() != config_vote.seed() -1 {
                error!("Received config_vote with wrong seed, msg={:?}", config_vote);
                return;
            }
        }

        let msg = config_vote.clone();
        let _ = view.config_votes().put(msg.from(), config_vote.clone());

        let mut votes_count = 0;
        for pub_key in self.state.validators(){
            if let Some(vote) = view.config_votes().get(&pub_key).unwrap() {
                if !vote.revoke() {
                    votes_count = votes_count + 1;
                }
            }
        }

        if votes_count >= 2/3 * self.state.validators().len(){
            if let Some(config_propose) = view.config_proposes().get(&config_vote.hash_propose()).unwrap() {
                view.configs().put(&Hash(self.height_to_slice(config_propose.actual_from_height())), config_propose.config().to_vec()).unwrap();
                // TODO: clear storages
            }
        }
    }

    fn height_to_slice(&self, height: u64) -> [u8;32] {
        let mut result = [0; 32];
        LittleEndian::write_u64(&mut result[24..], height);
        result
    }
}

#[cfg(test)]
mod tests {

    use super::super::super::crypto::{gen_keypair};
    use super::{Configuration, ConsensusCfg, NetworkCfg, ConfigurationValidator};

    #[test]
    fn validate_configuration(){
        // Arrange

        let (p1, _) = gen_keypair();
        let (p2, _) = gen_keypair();
        let (p3, _) = gen_keypair();

        let cfg = Configuration {
            version: 1,
            validators: vec![p1, p2, p3],
            consensus: ConsensusCfg {
                round_timeout: 2000,
                status_timeout: 5000,
                peers_timeout: 10000,
                propose_timeout: 500,
                txs_block_limit: 500
            }
        };

        // Assert
        assert_eq!(cfg.is_valid(), true);
    }

    #[test]
    fn deserialize_correct_configuration(){
        // Arrange
        let json = String::from("{\"actual_from\":1,\"validators\":[[255,110,239,100,242,107,33,125,149,196,6,71,45,5,143,15,66,144,168,233,171,18,1,81,183,253,49,72,248,226,88,224],[100,2,253,143,161,127,247,209,175,28,191,6,240,0,255,119,238,66,101,154,110,219,187,25,28,34,69,65,223,131,163,227],[185,187,188,22,223,202,133,226,118,76,203,52,17,132,193,213,117,57,36,15,106,67,129,218,175,32,34,235,240,51,83,81]],\"consensus\":{\"round_timeout\":2000,\"status_timeout\":5000,\"peers_timeout\":10000,\"propose_timeout\":500,\"txs_block_limit\":500}}").into_bytes().as_slice();

        // Act
        let cfg = Configuration::deserialize(&json);

        // Assert
        assert_eq!(cfg.is_ok(), true);

    }

    #[test]
    fn deserialize_wrong_configuration(){
        // Arrange
        let json = String::from("{\"actual_from\":1,\"validators\":[[255,110,239,100,242,107,33,125,149,196,6,71,45,5,143,15,66,144,168,233,171,18,1,81,183,253,49,72,248,226,88,224],[100,2,253,143,161,127,247,209,175,28,191,6,240,0,255,119,238,66,101,154,110,219,187,25,28,34,69,65,223,131,163,227],[185,187,188,22,223,202,133,226,118,76,203,52,17,132,193,213,117,57,36,15,106,67,129,218,175,32,34,235,240,51,83,81]],\"consensus\":{\"round_timeout\":11000,\"status_timeout\":5000,\"peers_timeout\":10000,\"propose_timeout\":500,\"txs_block_limit\":500}}").into_bytes().as_slice();

        // Act
        let cfg = Configuration::deserialize(json);

        // Assert
        assert_eq!(cfg.is_ok(), false);

    }


}