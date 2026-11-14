class V:
    x: int
    def __init__(self, x: int) -> None:
        self.x = x
    def __eq__(self, other: object) -> bool:
        if not isinstance(other, V):
            return NotImplemented
        return self.x == other.x
print(V(1) == V(1), V(1) == V(2))
