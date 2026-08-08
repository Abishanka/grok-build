#!/usr/bin/env python3
"""Serve contract fixtures on :7717 — Abi's TUI develops against this
with no brain, no keys, no George."""
import json
import os

from flask import Flask, jsonify, request

HERE = os.path.dirname(__file__)
FIX = os.path.join(HERE, "..", "contract", "fixtures")
app = Flask("fixtureserve")

with open(os.path.join(FIX, "cards_v1_all_kinds.json")) as f:
    CARDS = json.load(f)["cards"]
with open(os.path.join(FIX, "state_v1.json")) as f:
    STATE = json.load(f)


@app.get("/state")
def state():
    return jsonify(STATE)


@app.get("/cards")
def cards():
    since = int(request.args.get("since", 0))
    return jsonify({"cards": [c for c in CARDS if c["seq"] > since]})


@app.post("/action")
def action():
    return jsonify({"ok": True})


@app.get("/briefing.wav")
def briefing():
    return "fixture server has no audio", 503


if __name__ == "__main__":
    app.run(host="127.0.0.1", port=7717)
