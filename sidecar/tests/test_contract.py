"""Round-trip every contract fixture through core.Card — integration insurance."""
import json
import os

from brain.core import Card

FIXTURES = os.path.join(os.path.dirname(__file__), "..", "..", "contract", "fixtures")


def _cards(fname):
    with open(os.path.join(FIXTURES, fname)) as f:
        return json.load(f)["cards"]


def test_all_kinds_round_trip():
    for d in _cards("cards_v1_all_kinds.json"):
        card = Card.from_dict(d)
        assert card.to_dict() == d


def test_edge_round_trip():
    for d in _cards("cards_v1_edge.json"):
        assert Card.from_dict(d).to_dict() == d


def test_generated_cards_have_justification():
    for d in _cards("cards_v1_all_kinds.json"):
        if d["generated"]:
            assert d["justification"], f"{d['id']} generated without justification"
