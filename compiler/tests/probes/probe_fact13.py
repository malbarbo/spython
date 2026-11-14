def fat(n: int) -> int:
    r = 1
    for i in range(2, n + 1):
        r *= i
    return r
print(fat(13))
