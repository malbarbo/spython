def f(x: int | None = None) -> int:
    if x is None:
        return 0
    return x
print(f(), f(3))
