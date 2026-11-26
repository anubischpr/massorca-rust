#!/usr/bin/env python3
"""Rewrites the addresses of a pairing offer and prints the new offer (decision 0080, task A11).

The offer is `mop1.` followed by base64url JSON. The server lists the address it listens on; the Android emulator
reaches the runner at 10.0.2.2, so the job points the offer there. Only `addresses` changes: the secret, the server pin
and the expiry stay as the server made them. Reads the offer on stdin. Never echoes it, not even on failure.
"""

import argparse
import base64
import json
import sys

PREFIX = "mop1."


def decode(text):
    if not text.startswith(PREFIX):
        raise ValueError("not a pairing offer")
    body = text[len(PREFIX):]
    return json.loads(base64.urlsafe_b64decode(body + "=" * (-len(body) % 4)))


def encode(offer):
    raw = json.dumps(offer, separators=(",", ":")).encode()
    return PREFIX + base64.urlsafe_b64encode(raw).decode().rstrip("=")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--address", required=True, help="host:port the offer lists instead of the server's own")
    args = parser.parse_args()
    offer = decode(sys.stdin.read().strip())
    offer["addresses"] = [args.address]
    sys.stdout.write(encode(offer))


if __name__ == "__main__":
    try:
        main()
    except Exception:  # Any failure: say so, but never the offer text.
        sys.stderr.write("the pairing offer could not be rewritten\n")
        sys.exit(1)
