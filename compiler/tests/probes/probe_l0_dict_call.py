def f() -> dict[str, int]:
    d: dict[str, int] = dict()
    d["a"] = 1
    d.setdefault("b", 2)
    return d
print(f())
