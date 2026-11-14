class C:
    def __init__(self, x: int) -> None:
        self._x = x
    @property
    def x(self) -> int:
        return self._x
print(C(2).x)
