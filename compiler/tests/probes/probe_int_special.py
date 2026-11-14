class V:
    x: float
    def __init__(self, x: float) -> None:
        self.x = x
    def __int__(self) -> int:
        return 3
    def __float__(self) -> float:
        return 2.5
print(int(V(-2.0)), float(V(1.0)))
