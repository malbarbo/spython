def f(x: int | None) -> str:
    return f"valor: {x}"
print(f(None), f(2))
