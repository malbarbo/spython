class G:
    v: list[int]
    def __init__(self) -> None:
        self.v = [0, 0]
    def __setitem__(self, i: int, x: int) -> None:
        self.v[i] = x
    def __radd__(self, o: int) -> int:
        return o + sum(self.v)
    def __iadd__(self, o: int) -> "G":
        self.v.append(o)
        return self
g = G()
g[0] = 5
g += 3
print(g.v, 1 + g)
