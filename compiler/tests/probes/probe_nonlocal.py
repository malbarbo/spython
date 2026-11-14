def contador() -> int:
    n = 0
    def inc() -> None:
        nonlocal n
        n += 1
    inc()
    inc()
    return n
print(contador())
