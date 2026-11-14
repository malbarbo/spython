def f(n: int) -> int:
    def g(x: int, k: int = 2) -> int:
        return x * k
    return g(n)
print(f(3))
