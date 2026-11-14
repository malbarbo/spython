def f(xs: list[int], i: int) -> int:
    try:
        return xs[i] // xs[0]
    except (IndexError, ZeroDivisionError) as e:
        print(type(e).__name__)
        return 0
print(f([1, 2], 5))
print(f([0, 2], 1))
