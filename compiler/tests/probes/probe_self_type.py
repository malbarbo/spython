from typing import Self
class P:
    x: int
    def __init__(self, x: int) -> None:
        self.x = x
    def copia(self) -> Self:
        return self
print(P(1).copia().x)
