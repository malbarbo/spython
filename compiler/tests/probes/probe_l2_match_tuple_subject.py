def f(a: int, b: int) -> str:
    match a, b:
        case 0, 0:
            return "origem"
        case _:
            return "outro"
print(f(0, 0))
