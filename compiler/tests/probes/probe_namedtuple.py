from typing import NamedTuple
class P(NamedTuple):
    x: int
    y: int
p = P(1, 2)
print(p, p.x)
