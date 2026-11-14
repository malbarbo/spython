def f(x: int | str) -> bool:
    return isinstance(x, (int, float))
print(f(1), f("a"))
