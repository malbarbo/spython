class C:
    v: list[int]
    def __init__(self) -> None:
        self.v = [1, 2]
    def __delitem__(self, i: int) -> None:
        del self.v[i]
c = C()
del c[0]
print(c.v)
