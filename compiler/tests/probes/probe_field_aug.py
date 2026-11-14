from dataclasses import dataclass
@dataclass
class C:
    x: int
c = C(1)
c.x += 2
print(c)
