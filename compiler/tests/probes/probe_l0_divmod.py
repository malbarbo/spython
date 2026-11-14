def f(n: int) -> int:
    q, r = divmod(n, 10)
    return q + r
print(f(47), divmod(7, 2), "a=b".partition("="))
