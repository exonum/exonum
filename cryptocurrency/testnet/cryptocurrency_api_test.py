#!/usr/bin/env python3

import unittest
import requests
import json
import time
import base64
import random
import datetime

from exonum import ExonumApi, random_hex
        
class CryptocurrencyApi(ExonumApi):
    def create_user(self, name): 
        tx, c = self.send_transaction("wallets/create", {"name": name})
        return (self.find_user(c), c)

    def find_user(self, cookies):
        r = self.get("wallets/info", cookies)
        return r.json()

    def issue(self, cookies, amount):
        tx, _ = self.send_transaction("wallets/create", {"amount": amount}, cookies=cookies)
    


class CryptocurrencyApiTest(CryptocurrencyApi):
    def setUp(self):
        super().setUp()
        self.host = "http://127.0.0.1:8500/api/v1"
        self.times = 50

    def create_many_users(self, txs):
        cookies = []
        final_tx = None

        print()
        print(" - Create {} users".format(txs))
        start = datetime.datetime.now()
        for i in range(txs):
            r, c = self.post_transaction("wallets/create", {"name":"name_" + str(i)})
            cookies.append(c)
            final_tx = r["tx_hash"]

        tx = self.wait_for_transaction(final_tx)
        self.assertNotEqual(tx, None)
        finish = datetime.datetime.now()
        
        delta = finish - start
        ms = delta.seconds * 1000 + delta.microseconds / 1000
        print(" - Commited, txs={}, total time: {}ms".format(txs, ms))

        start = datetime.datetime.now()
        for i in range(txs):
            info = self.find_user(cookies[i])
            self.assertEqual(info["name"], "name_" + str(i))
        finish = datetime.datetime.now()
        
        delta = finish - start
        ms = delta.seconds * 1000 + delta.microseconds / 1000
        print(" - All users found, total time: {}ms".format(ms))

    def test_create_user(self):
        r, c = self.create_user("My First User")
        self.assertEqual(r["name"], "My First User")
        self.assertEqual(r["balance"], 0)

    def test_create_users_1_10(self):
        self.create_many_users(10)

    def test_create_users_2_100(self):
        self.create_many_users(100)

    def test_create_users_3_1000(self):
        self.create_many_users(1000)

    def test_create_users_4_5000(self):
        self.create_many_users(5000)

    def test_create_users_5_10000(self):
        self.create_many_users(10000)


if __name__ == '__main__':
    unittest.main(verbosity=2, buffer=None)
