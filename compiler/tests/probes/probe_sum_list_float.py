def media(xs: list[float]) -> float:
    return sum(xs) / len(xs)
def total(xs: list[float]) -> float:
    return sum(xs)
print(media([1.0, 2.0]), total([1.5]))
