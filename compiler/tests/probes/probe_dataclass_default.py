from dataclasses import dataclass
@dataclass
class P:
    x: int
    y: int = 0
print(P(1))
