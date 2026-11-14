from dataclasses import dataclass, field
@dataclass
class P:
    x: int
    n: int = field(init=False, repr=False, compare=False, default=0)
print(P(1), P(1) == P(1))
