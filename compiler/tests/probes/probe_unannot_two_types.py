class C:
    def __init__(self) -> None:
        self.total = 0
    def add(self, v: float) -> None:
        self.total = self.total + v
c = C()
c.add(1.5)
print(c.total)
