def f(n: int) -> int:
    if n == 0:
        return 0
    return 1 + f(n - 1)
print(f(5000))
