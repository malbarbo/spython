class C:
    def __init__(self) -> None:
        self.n = 0
    def reset(self) -> None:
        self.m = 1
c = C()
c.reset()
print(c.m)
