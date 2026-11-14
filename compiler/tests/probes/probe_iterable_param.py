from typing import Iterable
def soma(xs: Iterable[int]) -> int:
    s = 0
    for x in xs:
        s += x
    return s
print(soma([1, 2]), soma({3}), soma(range(4)))
