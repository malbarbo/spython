def f(xs: list[tuple[str, int]]) -> int:
    t = 0
    for _, n in xs:
        t += n
    return t
print(f([("a", 1)]))
