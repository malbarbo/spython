class C:
    x: int
    def __init__(self, x: int) -> None:
        self.x = x
    @classmethod
    def zero(cls) -> "C":
        return cls(0)
print(C.zero().x)
