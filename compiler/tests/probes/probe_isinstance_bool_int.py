def f(x: int | str) -> str:
    if isinstance(x, int):
        return "int"
    return "str"
print(f(True), f("a"))
