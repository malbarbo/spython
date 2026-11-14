def f(x: int | str) -> str:
    if isinstance(x, int):
        return "int"
    return x
print(f(1), f("a"))
