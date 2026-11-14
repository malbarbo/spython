def f(c: bool) -> int | str:
    return 1 if c else "a"
print(f(True), f(False))
