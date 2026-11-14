from dataclasses import dataclass
@dataclass
class P:
    x: int
print(vars(P(1)))
