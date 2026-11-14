def f(x: int) -> str:
    match x:
        case 0:
            return "zero"
        case 1 | 2:
            return "pouco"
        case _:
            return "muito"
print(f(0), f(2), f(9))
