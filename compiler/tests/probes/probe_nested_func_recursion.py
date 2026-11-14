def f(n: int) -> int:
    def g(k: int) -> int:
        if k == 0:
            return 0
        return k + g(k - 1)
    return g(n)
print(f(4))
