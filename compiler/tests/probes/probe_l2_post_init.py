from dataclasses import dataclass
@dataclass
class P:
    x: int
    def __post_init__(self) -> None:
        assert self.x > 0
print(P(1))
