from dataclasses import dataclass
@dataclass
class P:
    x: int
p = P(1)
print(p, p == P(1), p.x)
