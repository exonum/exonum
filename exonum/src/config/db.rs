use std::ops::Deref;

use super::super::crypto::{Hash, hash};
use messages::Message;
use ::storage::{Database, Error};
use storage::Map;
use ::blockchain::Blockchain;
use config::txs::{ConfigTx, TxConfigPropose, TxConfigVote };
use config::view::ConfigsView;
use config::height_to_slice;

use super::view::StoredConfiguration;

#[derive(Clone)]
pub struct ConfigsBlockchain<D: Database> {
    pub db: D,
    pub loaded_config: Option<StoredConfiguration>
}

impl<D: Database> Deref for ConfigsBlockchain<D> {
    type Target = D;
    fn deref(&self) -> &D {
        &self.db
    }
}

impl<D> ConfigsBlockchain<D> where D: Database {

    fn handle_config_propose(&self, config_propose: &TxConfigPropose) {
        
        let view = self.view();
        if let Some(ref config) = self.loaded_config {
            if !config.validators.contains(config_propose.from()){
                error!("ConfigPropose from unknown validator: {:?}", config_propose.from());
                return;            
            }
        }
        
        if view.config_proposes().get(&config_propose.hash()).unwrap().is_some() {
            error!("Received config_propose has already been handled, msg={:?}", config_propose);
            return;
        }

        trace!("Handle ConfigPropose");
        let _ = view.config_proposes().put(&config_propose.hash(), config_propose.clone());        

    }

    fn handle_config_vote(&self, config_vote: &TxConfigVote){        

        let view = self.view();
        if let Some(ref config) = self.loaded_config {

            let validators = config.validators.clone();
            if !validators.contains(config_vote.from()){
                error!("ConfigVote from unknown validator: {:?}", config_vote.from());
                return;
            }
        
            if view.config_proposes().get(config_vote.hash_propose()).unwrap().is_some() {
                error!("Received config_vote for unknown transaciton, msg={:?}", config_vote);
                return;
            }

            if let Some(vote) = view.config_votes().get(config_vote.from()).unwrap() {
                if vote.seed() != config_vote.seed() -1 {
                    error!("Received config_vote with wrong seed, msg={:?}", config_vote);
                    return;
                }
            }

            let msg = config_vote.clone();
            let _ = view.config_votes().put(msg.from(), config_vote.clone());


            let validators = config.validators.clone();
            let mut votes_count = 0;
            for pub_key in validators {
                if let Some(vote) = view.config_votes().get(&pub_key).unwrap() {
                    if !vote.revoke() {
                        votes_count += 1;
                    }
                }
            }

            let validators = config.validators.clone();
            if votes_count >= 2/3 * validators.len(){
                if let Some(config_propose) = view.config_proposes().get(config_vote.hash_propose()).unwrap() {
                    view.configs().put(&height_to_slice(config_propose.actual_from_height()), config_propose.config().to_vec()).unwrap();
                    // TODO: clear storages
                }
            }
        }
    }
}

impl<D> Blockchain for ConfigsBlockchain<D> where D: Database
{
    type Database = D;
    type Transaction = ConfigTx;
    type View = ConfigsView<D::Fork>;

    fn verify_tx(tx: &Self::Transaction) -> bool {
        tx.verify(tx.pub_key())
    }

    fn state_hash(view: &Self::View) -> Result<Hash, Error> {
        let mut hashes = Vec::new();
        hashes.extend_from_slice(view.configs().root_hash()?.as_ref());        
        Ok(hash(&hashes))
    }

    fn execute(&self, tx: &Self::Transaction) -> Result<(), Error> {
        match *tx {
            ConfigTx::ConfigPropose(ref tx) => {
                self.handle_config_propose(tx);                              
            }
            ConfigTx::ConfigVote(ref tx) => {
                self.handle_config_vote(tx);
                                
            }
        }
        Ok(())
    }    
    
    fn get_initial_configuration (&mut self) -> Option<StoredConfiguration> {
        let r = self.last_block().unwrap();
        let last_height = if let Some(last_block) = r {
            last_block.height() + 1
        } else {
            0
        };
        let mut h = last_height;

        while h > 0 {
            if let Some(configuration) = self.get_configuration_at_height(h){   
                self.loaded_config = Some(configuration);             
                return self.loaded_config.clone();
            }
            h -= 1;
        }
        None
    }

    fn get_configuration_at_height (&mut self, height: u64) -> Option<StoredConfiguration> {
        let view = self.view();
        let configs = view.configs();
        if let Ok(config) = configs.get(&height_to_slice(height)) {            
            match StoredConfiguration::deserialize(&config.unwrap()) {
                Ok(configuration) => {    
                    self.loaded_config = Some(configuration);                               
                    return self.loaded_config.clone();
                },
                Err(_) => {
                    error!("Can't parse found configuration at height: {}", height);
                }
            }
        }
        None
    }
}