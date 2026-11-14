from typing import Protocol
class TemArea(Protocol):
    def area(self) -> float: ...
class Q:
    def area(self) -> float:
        return 2.0
def total(xs: list[TemArea]) -> float:
    t = 0.0
    for x in xs:
        t = t + x.area()
    return t
print(total([Q()]))
