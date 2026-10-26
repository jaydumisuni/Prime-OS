#!/usr/bin/env python3
import importlib.util
import json
from pathlib import Path
import unittest
import uuid

s = importlib.util.spec_from_file_location("bridge", Path(__file__).with_name("bridge.py"))
b = importlib.util.module_from_spec(s)
s.loader.exec_module(b)

class BridgeTests(unittest.TestCase):
    def setUp(self):
        self.key = str(uuid.uuid4())
        self.apps = {self.key: ("Files", "thunar.desktop", "abcd")}
        self.launched = []
    def invoke(self, method, path, body=b""):
        return b.dispatch(method, path, body, self.apps, self.launched.append)
    def test_preview_not_primed(self):
        code, data = self.invoke("GET", "/v1/applications")
        self.assertEqual(code, 200)
        self.assertEqual(data["host_id"], b.HOST_SENTINEL)
        self.assertEqual(data["generation_id"], "UBUNTU_PREVIEW_NOT_PRIME")
        self.assertEqual(data["applications"][0]["compatibility"]["state"], "RECOGNIZED")
    def test_allowlist_and_rejection(self):
        body = json.dumps({"schema":"prime.shell-launch-request.v1", "application_id":self.key}).encode()
        self.assertEqual(self.invoke("POST", "/v1/shell/launch", body)[0], 202)
        self.assertEqual(self.launched, ["thunar.desktop"])
        wrong = json.dumps({"schema":"prime.shell-launch-request.v1", "application_id":str(uuid.uuid4())}).encode()
        self.assertEqual(self.invoke("POST", "/v1/shell/launch", wrong)[0], 403)
        self.assertEqual(self.launched, ["thunar.desktop"])
    def test_bad_schema(self):
        for body in (b"oops", b"{}", b'{"schema":"other"}'):
            self.assertEqual(self.invoke("POST", "/v1/shell/launch", body)[0], 400)
        self.assertFalse(self.launched)
    def test_core_mutations_disabled(self):
        self.assertEqual(self.invoke("POST", "/v1/system/power")[0], 403)
        self.assertEqual(self.invoke("GET", "/v1/storage")[0], 503)
    def test_launch_failure_is_not_success(self):
        body = json.dumps({"schema":"prime.shell-launch-request.v1", "application_id":self.key}).encode()
        def failure(_):
            raise OSError("Cannot launch")
        self.assertEqual(b.dispatch("POST", "/v1/shell/launch", body, self.apps, failure)[0], 503)

if __name__ == "__main__":
    unittest.main()
