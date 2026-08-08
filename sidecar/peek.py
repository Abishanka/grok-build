#!/usr/bin/env python3
"""Watch-mode card viewer — the demo surface until the TUI pane lands.
Usage: python peek.py [--url http://127.0.0.1:7717] [--watch 2]"""
import argparse
import sys
import time

import requests

COLORS = {"real": "\033[36m", "digest": "\033[33m", "render": "\033[35m",
          "draft-post": "\033[95m", "voice-briefing": "\033[32m"}
RESET = "\033[0m"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", default="http://127.0.0.1:7717")
    ap.add_argument("--watch", type=float, default=2.0)
    ap.add_argument("--kind", default="")
    args = ap.parse_args()

    seq = 0
    while True:
        try:
            r = requests.get(f"{args.url}/cards", params={"since": seq}, timeout=5)
            for c in r.json()["cards"]:
                seq = max(seq, c["seq"])
                if args.kind and c["kind"] != args.kind:
                    continue
                color = COLORS.get(c["kind"], "")
                tag = " [AI-generated]" if c["generated"] else ""
                print(f"{color}[{c['seq']} {c['kind']}]{RESET} {c['title']}{tag}")
                body = c["body"][:200].replace("\n", "\n  ")
                if body:
                    print(f"  {body}")
                if c["justification"]:
                    print(f"  \033[2m({c['justification']})\033[0m")
        except requests.RequestException as e:
            print(f"(brain unreachable: {e})", file=sys.stderr)
        time.sleep(args.watch)


if __name__ == "__main__":
    main()
