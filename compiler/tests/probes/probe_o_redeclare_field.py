class A:
    x: int | None
    def __init__(self) -> None:
        self.x = None
class B(A):
    x: int
    def __init__(self) -> None:
        self.x = 1
print(B().x)
