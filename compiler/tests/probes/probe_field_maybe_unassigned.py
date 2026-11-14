class C:
    n: int
    def __init__(self, b: bool) -> None:
        if b:
            self.n = 1
c = C(True)
print(c.n)
