def f(d: dict[str, int]) -> None:
    for k, v in sorted(d.items()):
        print(k, v)
f({"b": 1, "a": 2})
