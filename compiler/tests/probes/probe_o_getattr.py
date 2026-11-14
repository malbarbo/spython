from dataclasses import dataclass
@dataclass
class P:
    x: int
print(getattr(P(1), "x"))
