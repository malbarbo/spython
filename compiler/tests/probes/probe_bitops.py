def f(n: int) -> int:
    return (n & 3) | (n ^ 1)
print(f(5))
