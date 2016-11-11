#!/usr/bin/env python3

import unittest

import requests
from requests.packages.urllib3.util.retry import Retry
from requests.adapters import HTTPAdapter

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
        super().setUp()
        self.times = 10
        self.timeout = 1
        # Configure http session
        self.session = requests.Session()
        retries = Retry(total=10,
                backoff_factor=0.2,
                status_forcelist=[ 500, 502, 503, 504 ])
        adapter = requests.adapters.HTTPAdapter(max_retries=retries)
        self.session.mount("http://", adapter)

    def tearDown(self):
        super().tearDown()
        self.session.close()
        self.session = None
    
    def url(self, endpoint):
        return self.host + "/" + endpoint

    def put(self, endpoint, payload, cookies=None): 
        url = self.url(endpoint)    
        r = self.session.put(url, json=payload, cookies=cookies)
        return r

    def post(self, endpoint, payload, cookies=None): 
        url = self.url(endpoint)
        r = self.session.post(url, json=payload, cookies=cookies)
        return r

    def get(self, endpoint, cookies=None): 
        url = self.url(endpoint)
        r = self.session.get(url, cookies=cookies)
        return r

    def find_transaction(self, hash): 
        endpoint = "blockchain/transactions/" + hash
        r = self.get(endpoint)
        if r.status_code == 200:
            return r.json()
        return None

    def post_transaction(self, endpoint, payload, cookies=None):
        times = 0
        r = None
        while times < self.times: 
            r = self.post(endpoint, payload, cookies)
            if r.status_code != 503:
                break
            time.sleep(self.timeout)
            times = times + 1
        return (r.json(), r.cookies)

    def put_transaction(self, endpoint, payload, cookies=None):
        times = 0
        r = None
        while times < self.times: 
            r = self.put(endpoint, payload, cookies)
            if r.status_code != 503:
                break
            time.sleep(self.timeout)
            times = times + 1
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