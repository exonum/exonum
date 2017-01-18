#!/usr/bin/env python3

import unittest
import datetime

from exonum import ExonumApi, random_hex

class ConfigsApi(ExonumApi):
    
    def new_config_propose(self, config, height, actual_from_height):
        tx, c = self.send_transaction("config/propose", {"config": config, "height": height, "actual_from_height": actual_from_height})
        return (self.get_config_propose(tx))

    def new_config_vote(self):
        tx, _ = self.send_transaction(
            "config/vote", {"config_propose_hash": hash})

    def get_config_propose(self, hash):
        r = self.get("config/propose/" + hash)
        return r.json()

    def get_config_vote(self, pubkey):
        r = self.get("config/vote/" + hash)
        return r.json()

class ConfigsApiTest(ConfigsApi):
    
    def setUp(self):
        super().setUp()
        self.host = "http://127.0.0.1:8400/api/v1"
        self.times = 120

    def create_many_proposes(self, txs):        
        final_tx = None

        print()
        print(" - Create {} config_proposes".format(txs))
        start = datetime.datetime.now()
        for i in range(txs):
            r, c = self.post_transaction(
                "wallets/create", {"name": "name_" + str(i)})            
            final_tx = r["tx_hash"]

        tx = self.wait_for_transaction(final_tx)
        self.assertNotEqual(tx, None)
        finish = datetime.datetime.now()

        delta = finish - start
        ms = delta.seconds * 1000 + delta.microseconds / 1000
        print(" - Commited, txs={}, total time: {}s".format(txs, ms / 1000))

        start = datetime.datetime.now()
        for i in range(txs):
            info = self.find_user(cookies[i])
            self.assertEqual(info["name"], "name_" + str(i))
        finish = datetime.datetime.now()

        delta = finish - start
        ms = delta.seconds * 1000 + delta.microseconds / 1000
        print(" - All users found, total time: {}s".format(ms / 1000))

    def test_create_config_propose(self):
        r, c = self.create_user("My First User")
        self.assertEqual(r["name"], "My First User")
        self.assertEqual(r["balance"], 0)

    def test_create_proposes_1_10(self):
        self.create_many_proposes(10)

    def test_create_proposes_2_100(self):
        self.create_many_proposes(100)

    def test_create_proposes_3_1000(self):
        self.create_many_proposes(1000)

    def test_create_proposes_4_5000(self):
        self.create_many_proposes(5000)

    def test_create_proposes_5_10000(self):
        self.create_many_proposes(10000)


if __name__ == '__main__':
    unittest.main(verbosity=2, buffer=None)
