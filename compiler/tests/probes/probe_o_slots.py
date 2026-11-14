class C:
    __slots__ = ("x",)
    def __init__(self) -> None:
        self.x = 1
print(C().x)
