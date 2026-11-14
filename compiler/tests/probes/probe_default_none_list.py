def f(xs: list[int] | None = None) -> list[int]:
    if xs is None:
        xs = []
    xs.append(1)
    return xs
print(f(), f([2]))
