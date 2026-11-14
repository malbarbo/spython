from dataclasses import dataclass
@dataclass
class P:
    x: int
delattr(P(1), "x")
