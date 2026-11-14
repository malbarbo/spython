def f(x: int | str) -> int:
    assert isinstance(x, int)
    return x
print(f(1))
