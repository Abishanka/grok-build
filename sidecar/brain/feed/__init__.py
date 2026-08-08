"""Lane B: card store (+ curator/ranker to come). Store seeded by integrator."""
import threading

from ..core import Card


class CardStore:
    def __init__(self):
        self._lock = threading.Lock()
        self._cards: list[Card] = []
        self._seq = 0

    def add(self, **kw) -> Card:
        with self._lock:
            self._seq += 1
            card = Card(seq=self._seq, id=kw.pop("id", f"c{self._seq}"), **kw)
            self._cards.append(card)
            return card

    def since(self, seq: int) -> list[Card]:
        with self._lock:
            return [c for c in self._cards if c.seq > seq]
