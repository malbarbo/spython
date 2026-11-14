class C:
    n: str
    def __init__(self, n: str) -> None:
        self.n = n
    def __enter__(self) -> str:
        print("in", self.n)
        return self.n
    def __exit__(self, a: object, b: object, c: object) -> None:
        print("out", self.n)
with C("a") as a, C("b") as b:
    print(a, b)
