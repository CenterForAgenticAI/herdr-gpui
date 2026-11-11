"""Exercise the installed POSIX shim against a deterministic core CLI peer."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SHIM = Path(__file__).resolve().parents[2] / "crates/sheprd-gpui/src/remote_browser/herdr-gpui.sh"
PEER = r'''
import json, os, pathlib, sys
home = pathlib.Path(os.environ["HOME"])
path = home / "labels.json"
labels = json.loads(path.read_text()) if path.exists() else {}
args = sys.argv[1:]
assert args[1] == "t3", args
with (home / "calls.jsonl").open("a") as out:
    out.write(json.dumps(args) + "\n")
if args[0] == "label":
    for pair in args[2:]:
        key, value = pair.split("=", 1)
        if value:
            labels[key] = value
        else:
            labels.pop(key, None)
    path.write_text(json.dumps(labels))
elif args[0] == "get":
    mode = os.environ.get("MODE", "notes")
    if mode == "disconnect":
        sys.exit(1)
    for key, value in list(labels.items()):
        if value.startswith("feedback-"):
            reply = "sheprd-reply-" + key[len("herdr-gpui-"):]
            if mode == "cancel":
                labels.pop(key)
            else:
                labels[reply] = json.dumps({"notes": None if mode == "empty" else "Remote café\nsecond line"})
    path.write_text(json.dumps(labels))
    print(json.dumps({"address": "t3", "state": "live", "labels": labels}))
'''


class FeedbackProtocol(unittest.TestCase):
    def run_shim(self, mode="notes", args=()):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            binary = home / ".local/bin/herdr-core"
            binary.parent.mkdir(parents=True)
            binary.write_text(f"#!{sys.executable}\n" + PEER)
            binary.chmod(0o755)
            env = {"HOME": directory, "PATH": os.path.dirname(sys.executable) + ":/usr/bin:/bin",
                   "HC_TERMINAL_ID": "t3", "MODE": mode}
            result = subprocess.run(["/bin/sh", str(SHIM), "browser", "feedback", "--json", *args],
                                    env=env, capture_output=True, text=True, timeout=5)
            calls = home / "calls.jsonl"
            labels = home / "labels.json"
            return (result, [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else [],
                    json.loads(labels.read_text()) if labels.exists() else {})

    def test_returns_unicode_multiline_notes_and_acknowledges_exact_request(self):
        result, calls, labels = self.run_shim(args=("--wait", "600"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {"notes": "Remote café\nsecond line"})
        key = calls[0][2].split("=", 1)[0]
        self.assertEqual(calls[0][2], key + "=feedback-600")
        self.assertEqual(calls[-1], ["label", "t3", key + "=ack"])
        self.assertEqual(labels[key], "ack")

    def test_empty_reply_uses_cli_no_notes_exit_status(self):
        result, _, _ = self.run_shim("empty")
        self.assertEqual(result.returncode, 4, result.stderr)
        self.assertEqual(json.loads(result.stdout), {"notes": None})

    def test_disconnect_and_cancellation_do_not_acknowledge_notes(self):
        for mode in ("disconnect", "cancel"):
            with self.subTest(mode=mode):
                result, calls, labels = self.run_shim(mode)
                self.assertEqual(result.returncode, 1)
                self.assertIn("error", json.loads(result.stdout))
                self.assertFalse(any(arg.endswith("=ack") for call in calls for arg in call))
                self.assertEqual(labels, {})

    def test_invalid_wait_never_contacts_core(self):
        for wait in ("601", "-1", "NaN"):
            result, calls, _ = self.run_shim(args=("--wait", wait))
            self.assertEqual(result.returncode, 2)
            self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
