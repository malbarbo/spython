def f(s: str) -> int:
    try:
        return int(s)
    except ValueError:
        return -1
print(f("a"), f("3"))
