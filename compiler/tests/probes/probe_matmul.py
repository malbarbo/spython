class M:
    v: int
    def __init__(self, v: int) -> None:
        self.v = v
    def __matmul__(self, o: "M") -> int:
        return self.v * o.v
print(M(2) @ M(3))
