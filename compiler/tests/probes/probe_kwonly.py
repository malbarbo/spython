def f(a: int, *, b: int = 2) -> int:
    return a + b
print(f(1, b=3))
