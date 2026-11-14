def f(n: int) -> list[int]:
    r: list[int] = []
    for i in range(n):
        r.append(i * i)
    return r
print(f(4))
