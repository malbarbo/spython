from dataclasses import dataclass
@dataclass
class C:
    x: int
    def __post_init__(self) -> None:
        if self.x < 0:
            raise ValueError("neg")
print(C(1))
