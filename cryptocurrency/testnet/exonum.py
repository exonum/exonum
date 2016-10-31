#!/usr/bin/env python3

import unittest
import requests
import json
import time
import base64
import random

def random_bytes(n):
    return bytearray(random.getrandbits(8) for i in range(n))

def random_hex(n=32):
    b = random_bytes(n)
    return base64.b16encode(b).decode("latin1").lower() 

class ExonumApi(unittest.TestCase):
    def setUp(self):
        self.times = 100
        self.timeout = 1
    
    def url(self, endpoint):
        return self.host + "/" + endpoint

    def put(self, endpoint, payload, cookies=None): 
        url = self.url(endpoint)    
        r = requests.put(url, json=payload, cookies=cookies)
        return r

    def post(self, endpoint, payload, cookies=None): 
        url = self.url(endpoint)
        r = requests.post(url, json=payload, cookies=cookies)
        return r

    def get(self, endpoint, cookies=None): 
        url = self.url(endpoint)
        r = requests.get(url, cookies=cookies)
        return r

    def find_transaction(self, hash): 
        endpoint = "blockchain/transactions/" + hash
        r = self.get(endpoint)
        if r.status_code == 200:
            return r.json()
        return None

    def post_transaction(self, endpoint, payload, cookies=None):
        r = self.post(endpoint, payload, cookies)
        return (r.json(), r.cookies)

    def put_transaction(self, endpoint, payload, cookies=None):
        r = self.put(endpoint, payload, cookies)
        return (r.json(), r.cookies)

    def wait_for_transaction(self, hash):
        times = 0
        r = None
        while times < self.times: 
            r = self.find_transaction(hash)
            if r != None:
                break
            time.sleep(self.timeout)
            times = times + 1
        return r

    def send_transaction(self, endpoint, payload, cookies=None, method="post"):
        if method == "post":
            r, c = self.post_transaction(endpoint, payload, cookies)
        elif method == "put":
            r, c = self.put_transaction(endpoint, payload, cookies)
        else: raise Exception("Unknown send tx method")

        hash = r["tx_hash"]
        return (self.wait_for_transaction(hash), c)