from dataclasses import dataclass
@dataclass
class P:
    i: int
xs = [P(1), P(2)]
print(min(p.i for p in xs))
