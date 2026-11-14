class V:
    x: float
    def __init__(self, x: float) -> None:
        self.x = x
    def __abs__(self) -> float:
        return abs(self.x)
print(abs(V(-2.0)))
