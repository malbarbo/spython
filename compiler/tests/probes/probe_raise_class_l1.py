def f(x: int) -> int:
    if x < 0:
        raise ValueError
    return x
print(f(-1))
