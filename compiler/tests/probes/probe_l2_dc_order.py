from dataclasses import dataclass, field, replace
@dataclass(order=True)
class P:
    x: int
    y: int = field(default=0)
print(P(1) < P(2), replace(P(1), y=3))
