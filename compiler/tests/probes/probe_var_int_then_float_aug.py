def f(xs: list[float]) -> float:
    t = 0
    for x in xs:
        t += x
    return t
print(f([1.5]))
