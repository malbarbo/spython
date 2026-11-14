def acha(xs: list[int], v: int) -> int | None:
    for i in range(len(xs)):
        if xs[i] == v:
            return i
    return None
print(acha([1, 2], 2), acha([1], 5))
