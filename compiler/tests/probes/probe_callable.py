def f(x: int | str) -> bool:
    return callable(x)
print(f(1), callable(print))
