def f(x: float) -> float:
    return abs(x) + max(x, 0.0) + min(1.0, x)
print(f(-1.0))
