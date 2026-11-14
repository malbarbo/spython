def f(s: str) -> str:
    match s.split():
        case [a, b]:
            return b + a
        case _:
            return s
print(f("x y"))
