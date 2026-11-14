from dataclasses import dataclass
@dataclass
class P:
    x: int
print(hasattr(P(1), "x"))
