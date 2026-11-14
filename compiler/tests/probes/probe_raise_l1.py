def f(x: int) -> int:
    if x < 0:
        raise ValueError(f"negativo: {x}")
    return x
print(f(-1))
