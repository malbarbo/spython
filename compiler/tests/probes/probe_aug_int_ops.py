def f() -> None:
    n = 7
    n //= 2
    n **= 3
    n %= 5
    n <<= 2
    n >>= 1
    n &= 6
    n |= 1
    n ^= 3
    s = "ab"
    s *= 2
    print(n, s)
f()
