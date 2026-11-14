class V:
    x: float
    def __init__(self, x: float) -> None:
        self.x = x
    def __add__(self, o: "V") -> "V":
        return V(self.x + o.x)
    def __getitem__(self, i: int) -> float:
        return self.x * i
    def __len__(self) -> int:
        return 3
    def __contains__(self, v: float) -> bool:
        return v == self.x
    def __bool__(self) -> bool:
        return self.x != 0
    def __neg__(self) -> "V":
        return V(-self.x)
print((V(1.0) + V(2.0)).x, V(2.0)[3], len(V(1.0)), 1.0 in V(1.0), bool(V(0.0)), (-V(1.0)).x)
