def soma(m: list[list[int]]) -> int:
    s = 0
    for linha in m:
        for x in linha:
            s += x
    return s
print(soma([[1, 2], [3]]))
