def f(n: int) -> int:
    return f(n + 1)
try:
    f(0)
except RecursionError:
    print("rec")
