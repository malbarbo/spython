def f(xs: list[int]) -> None:
    for x in xs:
        if x < 0:
            break
    else:
        print("todos")
    i = 0
    while i < 2:
        i += 1
    else:
        print("fim")
f([1])
