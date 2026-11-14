class A:
    n: int
    def __init__(self) -> None:
        self.n = 1
    def m(self) -> int:
        return self.n
a = A()
f = a.m
print(f())
