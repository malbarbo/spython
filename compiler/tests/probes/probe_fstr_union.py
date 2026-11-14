def f(x: int | str) -> str:
    return f"v={x}"
print(f(1), f("a"))
