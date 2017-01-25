#!/usr/bin/env python3

import unittest
import datetime

from exonum import ExonumApi, random_hex

class ConfigsApiTest(ExonumApi):
    
    config_propose_hash = ''
    
    def setUp(self):
        super().setUp()
        self.host = "http://127.0.0.1:8900/api/v1"
        self.times = 120

    def step1_test_create_propose(self):        
        print()
        print (" - Create config_propose")
        start = datetime.datetime.now()
        r, _ = self.put_transaction("configs/propose", 
        { "actual_from": 1,
          "validators":[],
          "consensus": {
            "round_timeout":2,
            "status_timeout": 3,
            "peers_timeout": 4,
            "propose_timeout": 5,
            "txs_block_limit": 6
            } 
        })
        print(r)
        self.config_propose_hash = r["tx_hash"]
        trx = self.wait_for_transaction(self.config_propose_hash, "configs/propose/")
        self.assertNotEqual(trx, None)
        self.assertEqual(trx['config']['actual_from'], 1)
        self.assertEqual(trx['config']['consensus']['round_timeout'], 2)
        self.assertEqual(trx['config']['consensus']['status_timeout'], 3)
        self.assertEqual(trx['config']['consensus']['peers_timeout'], 4)
        self.assertEqual(trx['config']['consensus']['propose_timeout'], 5)
        self.assertEqual(trx['config']['consensus']['txs_block_limit'], 6)

    # def step2_test_vote_for_unknown_propose(self):        
    #     print()
    #     print (" - Create config_vote for unknown")
    #     start = datetime.datetime.now()
    #     r, _ = self.put_transaction("configs/vote", 
    #     { 
    #         "height": 1,
    #         "hash_propose": "3222222222222222222222222222222222222222222222222222222222222222",
    #         "seed": 2,
    #         "revoke": False
    #     })        
    #     tx_hash = r.get('tx_hash')
    #     self.assertNotEqual(tx_hash, None)
    #     trx = self.wait_for_transaction(tx_hash, 5)
    #     self.assertEqual(trx, None)

    def step4_test_vote_for_known_propose(self):        
        print()
        print (" - Create config_vote for known")
        start = datetime.datetime.now()
        r, _ = self.put_transaction("configs/vote", 
        { 
            "height": 1,
            "hash_propose": self.config_propose_hash,
            "seed": 4,
            "revoke": False
        })        

        tx_hash = r["tx_hash"]
        trx = self.wait_for_transaction("393db95d6f03db824460752e93bee50c231d5b39cfeb08acf0f1a058bf21eba1", "configs/vote/", 5)
        self.assertNotEqual(trx, None) 
        self.assertFalse(trx['revoke'])

    def step5_test_revoke_for_known_propose(self):        
        print()
        print (" - Create config_vote revoke for known")
        start = datetime.datetime.now()
        r, _ = self.put_transaction("configs/vote", 
        { 
            "height": 1,
            "hash_propose": self.config_propose_hash,
            "seed": 5,
            "revoke": True
        })        

        tx_hash = r["tx_hash"]
        trx = self.wait_for_transaction("393db95d6f03db824460752e93bee50c231d5b39cfeb08acf0f1a058bf21eba1", "configs/vote/", 5)
        self.assertNotEqual(trx, None) 
        self.assertTrue(trx['revoke'])

    def _steps(self):
        for name in sorted(dir(self)):
            if name.startswith("step"):
                yield name, getattr(self, name) 

    def test_steps(self):
        for name, step in self._steps():
            # try:
                step()
            # except Exception as e:
                # self.fail("{} failed ({}: {})".format(step, type(e), e))    

if __name__ == '__main__':
    unittest.main(verbosity=2, buffer=None)
