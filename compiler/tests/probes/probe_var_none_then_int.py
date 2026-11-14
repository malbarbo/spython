def f(xs: list[int]) -> int:
    m = None
    for x in xs:
        if m is None or x > m:
            m = x
    if m is None:
        return 0
    return m
print(f([3, 1]))
