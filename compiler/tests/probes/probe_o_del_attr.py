class C:
    x: int
    def __init__(self) -> None:
        self.x = 1
c = C()
del c.x
print(1)
