"""FROZEN — integrator-owned. Contract types. Change = both humans ack."""
from dataclasses import dataclass, asdict, field

KINDS = ("real", "digest", "render", "draft-post", "voice-briefing")


@dataclass
class Card:
    id: str
    seq: int
    kind: str
    title: str
    body: str
    media_url: str = ""
    source_url: str = ""
    generated: bool = False
    justification: str = ""
    diff_context: str = ""

    def __post_init__(self):
        if self.kind not in KINDS:
            raise ValueError(f"bad kind: {self.kind}")

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "Card":
        return cls(**d)
