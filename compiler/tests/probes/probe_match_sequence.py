def f(xs: list[int]) -> str:
    match xs:
        case []:
            return "vazia"
        case [x]:
            return f"um {x}"
        case [x, *resto]:
            return f"{x} e {len(resto)}"
    return "?"
print(f([]), f([1]), f([1, 2, 3]))
