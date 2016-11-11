#!/usr/bin/env python3

import unittest
import pprint

from exonum import ExonumApi, random_hex

class DigitalRightsApi(ExonumApi):

    def find_user(self, pub_key, cookies):
        r = self.get("drm/find_user/" + pub_key, cookies)
        return r.json()

    def owner_info(self, id):
        return self.get("drm/owners/" + str(id)).json()

    def distributor_info(self, id):
        return self.get("drm/distributors/" + str(id)).json()

    def content_info(self, fingerprint, cookies):
        return self.get("drm/contents/" + fingerprint, cookies).json()

    def create_owner(self, name):
        tx, c = self.send_transaction("drm/owners", {"name": name})
        user = self.find_user(tx["pub_key"], c)
        return {"info": user, "cookies": c}

    def create_distributor(self, name):
        tx, c = self.send_transaction("drm/distributors", {"name": name})
        user = self.find_user(tx["pub_key"], c)
        return {"info": user, "cookies": c}

    def add_content(self, content, cookies):
        tx, c = self.send_transaction(
            "drm/contents", content, cookies, method="put")
        return tx

    def add_contract(self, id, fingerprint, cookies):
        endpoint = "drm/contracts/" + fingerprint
        tx, c = self.send_transaction(
            endpoint, payload=None, cookies=cookies, method="put")
        return tx

    def report(self, report, cookies):
        tx, c = self.send_transaction(
            "drm/reports", report, cookies, method="put")
        return tx


class DigitalRightsApiTest(DigitalRightsApi):

    def setUp(self):
        super().setUp()
        self.host = "http://127.0.0.1:8600/api/v1"
        self.times = 60
        self.timeout = 1

    def test_create_owner(self):
        owner = self.create_owner("Unknown Artist")
        info = self.owner_info(owner["info"]["id"])
        self.assertEqual(info["name"], "Unknown Artist")

    def test_create_distributor(self):
        distributor = self.create_distributor("Bitfury Music")
        info = self.distributor_info(distributor["info"]["id"])
        self.assertEqual(info["name"], "Bitfury Music")

    def test_complex(self):
        pp = pprint.PrettyPrinter(indent=4)

        owner0 = self.create_owner("Unknown Artist")
        owner1 = self.create_owner("Garage Band")

        content0 = {
            "title": "Unknown Album - Track 1",
            "fingerprint": random_hex(),
            "additional_conditions": "",
            "price_per_listen": 1,
            "min_plays": 100,
            "owners": [
                {"owner_id": owner0["info"]["id"], "share":60},
                {"owner_id": owner1["info"]["id"], "share":40}
            ]
        }
        content1 = {
            "title": "Unknown Album - Track 2",
            "fingerprint": random_hex(),
            "additional_conditions": "",
            "price_per_listen": 10,
            "min_plays": 100,
            "owners": [
                {"owner_id": owner0["info"]["id"], "share":5},
                {"owner_id": owner1["info"]["id"], "share":95}
            ]
        }

        tx = self.add_content(content0, owner0["cookies"])
        self.assertNotEqual(tx, None)
        tx = self.add_content(content1, owner1["cookies"])
        self.assertNotEqual(tx, None)

        distributor0 = self.create_distributor("Exonum Entertaiment")
        distributor1 = self.create_distributor("Bitfury Music")

        tx = self.add_contract(
            distributor0["info"]["id"],
            content0["fingerprint"],
            distributor0["cookies"]
        )
        self.assertNotEqual(tx, None)

        tx = self.add_contract(
            distributor0["info"]["id"],
            content1["fingerprint"],
            distributor0["cookies"]
        )
        self.assertNotEqual(tx, None)

        tx = self.add_contract(
            distributor1["info"]["id"],
            content0["fingerprint"],
            distributor1["cookies"]
        )
        self.assertNotEqual(tx, None)

        tx = self.add_contract(
            distributor1["info"]["id"],
            content1["fingerprint"],
            distributor1["cookies"]
        )
        self.assertNotEqual(tx, None)

        reports = [
            {
                "uuid": random_hex(),
                "fingerprint": content0["fingerprint"],
                "time": 1000,
                "plays": 100,
                "comment": "My First report"
            },
            {
                "uuid": random_hex(),
                "fingerprint": content0["fingerprint"],
                "time": 1500,
                "plays": 200,
                "comment": "My Second report"
            },
            {
                "uuid": random_hex(),
                "fingerprint": content1["fingerprint"],
                "time": 2000,
                "plays": 300,
                "comment": "My Third report"
            },
            {
                "uuid": random_hex(),
                "fingerprint": content0["fingerprint"],
                "time": 3000,
                "plays": 400,
                "comment": "My Fourth report"
            },
        ]

        self.assertNotEqual(self.report(
            reports[0], distributor0["cookies"]), None)
        self.assertNotEqual(self.report(
            reports[1], distributor1["cookies"]), None)
        self.assertNotEqual(self.report(
            reports[2], distributor0["cookies"]), None)
        self.assertNotEqual(self.report(
            reports[3], distributor0["cookies"]), None)

        distributors_info = [
            [
                self.content_info(
                    content0["fingerprint"], distributor0["cookies"]),
                self.content_info(
                    content1["fingerprint"], distributor0["cookies"])
            ],
            [
                self.content_info(
                    content0["fingerprint"], distributor1["cookies"]),
                self.content_info(
                    content1["fingerprint"], distributor1["cookies"])
            ]
        ]
        owners_info = [
            [
                self.content_info(content0["fingerprint"], owner0["cookies"]),
                self.content_info(content1["fingerprint"], owner0["cookies"])
            ],
            [
                self.content_info(content0["fingerprint"], owner1["cookies"]),
                self.content_info(content1["fingerprint"], owner1["cookies"])
            ]
        ]

        self.assertEqual(distributors_info[0][0]["contract"]["amount"], 500)
        self.assertEqual(distributors_info[0][1]["contract"]["amount"], 3000)
        self.assertEqual(distributors_info[1][0]["contract"]["amount"], 200)
        self.assertEqual(distributors_info[1][1]["contract"]["amount"], 0)

        self.assertEqual(owners_info[0][0]["amount"], 420)
        self.assertEqual(owners_info[1][0]["amount"], 280)
        self.assertEqual(owners_info[0][1]["amount"], 150)
        self.assertEqual(owners_info[1][1]["amount"], 2850)

    def test_add_content(self):
        pp = pprint.PrettyPrinter(indent=4)

        owner = self.create_owner("Unknown Artist")
        contents = [
            {
                "title": "Unknown Album - Track 1",
                "fingerprint": random_hex(),
                "additional_conditions": "",
                "price_per_listen": 1,
                "min_plays": 100,
                "owners": [
                    {"owner_id": owner["info"]["id"], "share":100},
                ]
            },
            {
                "title": "Unknown Album - Track 2",
                "fingerprint": random_hex(),
                "additional_conditions": "",
                "price_per_listen": 25,
                "min_plays": 10000,
                "owners": [
                    {"owner_id": owner["info"]["id"], "share":100},
                ]
            }
        ]
        tx = self.add_content(contents[0], owner["cookies"])
        self.assertNotEqual(tx, None)
        
        print("Get content info for owner")
        info = self.content_info(contents[0]["fingerprint"], owner["cookies"])
        pp.pprint(info)
        print("Get content info for unregistred user")
        info = self.content_info(contents[0]["fingerprint"], None)
        pp.pprint(info)

        tx = self.add_content(contents[1], owner["cookies"])
        self.assertNotEqual(tx, None)

        print("Get content info for owner")        
        info = self.content_info(contents[1]["fingerprint"], owner["cookies"])
        pp.pprint(info)
        print("Get content info for unregistred user")        
        info = self.content_info(contents[1]["fingerprint"], None)
        pp.pprint(info)


if __name__ == '__main__':
    unittest.main()
