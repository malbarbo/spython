def f(s: str) -> int:
    match s:
        case "a":
            return 1
        case _:
            return 0
print(f("a"))
