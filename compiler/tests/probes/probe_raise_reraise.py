def f() -> None:
    try:
        raise ValueError("a")
    except ValueError:
        print("vi")
        raise
try:
    f()
except ValueError as e:
    print("de novo", e)
