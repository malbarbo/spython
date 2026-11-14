from typing import ClassVar
class P:
    total: ClassVar[int] = 0
    def __init__(self) -> None:
        P.total += 1
P()
P()
print(P.total)
