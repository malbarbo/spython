from dataclasses import dataclass
@dataclass(order=True)
class C:
    x: int
print(C(1) < C(2) < C(3))
