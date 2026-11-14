from dataclasses import dataclass, InitVar
@dataclass
class P:
    x: int
    k: InitVar[int]
    def __post_init__(self, k: int) -> None:
        self.x *= k
print(P(1, 3))
