def f() -> int:
    class P:
        x: int
        def __init__(self) -> None:
            self.x = 3
    return P().x
print(f())
