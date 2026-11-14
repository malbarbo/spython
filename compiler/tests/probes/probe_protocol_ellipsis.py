from typing import Protocol
class TemArea(Protocol):
    def area(self) -> float: ...
class Q:
    def area(self) -> float:
        return 2.0
def dobro(x: TemArea) -> float:
    return x.area() * 2.0
print(dobro(Q()))
