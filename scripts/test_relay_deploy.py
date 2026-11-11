"""The relay's DigitalOcean user data, rendered the way create-droplet.sh
renders it, picks the domain it was given, or IP.sslip.io without one."""

import os
import re
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLOUD_INIT = os.path.join(ROOT, "deploy", "relay", "cloud-init.yml")


def rendered(domain: str) -> str:
    """cloud-init.yml after create-droplet.sh's sed substitution."""
    with open(CLOUD_INIT, encoding="utf-8") as file:
        text = file.read()
    return (
        text.replace("__COMPOSE_B64__", "Y29tcG9zZQ==")
        .replace("__CADDYFILE_B64__", "Y2FkZHk=")
        .replace("__RELAY_DOMAIN__", domain)
    )


def chosen_domain(text: str) -> str:
    """Runs start.sh's domain choice, with the metadata service answering
    203.0.113.7, and returns the domain it settles on."""
    block = re.search(r'\n(\s*)domain="[^"\n]*"\n.*?\n\1fi\n', text, re.S)
    assert block, "start.sh's domain block is missing"
    script = "\n".join(line.strip() for line in block.group(0).splitlines())
    shell = 'curl() { echo 203.0.113.7; }\n' + script + '\nprintf "%s" "$domain"\n'
    with tempfile.TemporaryDirectory() as directory:
        path = os.path.join(directory, "domain.sh")
        with open(path, "w", encoding="utf-8") as file:
            file.write(shell)
        return subprocess.run(
            ["sh", path], check=True, capture_output=True, text=True
        ).stdout


class DomainTest(unittest.TestCase):
    def test_a_given_domain_is_kept(self) -> None:
        self.assertEqual(
            chosen_domain(rendered("nyc3.relay.example.org")), "nyc3.relay.example.org"
        )

    def test_no_domain_falls_back_to_sslip(self) -> None:
        self.assertEqual(chosen_domain(rendered("")), "203-0-113-7.sslip.io")

    def test_an_unfilled_placeholder_falls_back_to_sslip(self) -> None:
        with open(CLOUD_INIT, encoding="utf-8") as file:
            self.assertEqual(chosen_domain(file.read()), "203-0-113-7.sslip.io")


if __name__ == "__main__":
    unittest.main()
