def soma(t: tuple[int, ...]) -> int:
    s = 0
    for x in t:
        s += x
    return s
print(soma((1, 2, 3)))
