from dataclasses import dataclass
@dataclass(eq=False)
class P:
    x: int
print(P(1) == P(1))
