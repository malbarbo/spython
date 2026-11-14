class C:
    def __init__(self, b: bool) -> None:
        if b:
            self.n = 1
c = C(False)
print(c.n)
