from dataclasses import dataclass
@dataclass
class P:
    x: int
    y: int
print(P(1, 2) == P(1, 2), P(1, 2))
