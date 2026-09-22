"""Encode an NMP/1 Ping envelope; transport and authentication are separate."""
import json
import re
import sys

def uint(value):
    return re.fullmatch(r"0|[1-9][0-9]{0,19}", value) is not None and int(value) <= 18446744073709551615

args = sys.argv[1:]
if len(args) != 3 or re.fullmatch(r"[0-9a-fA-F]{64}", args[0]) is None or not all(map(uint, args[1:])) or args[1] == "0":
    sys.exit("expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)")
print(json.dumps({"version": 1, "message_id": int(args[1]), "sender": list(bytes.fromhex(args[0])), "message": {"type": "Ping", "body": {"nonce": int(args[2])}}}, separators=(",", ":")))
