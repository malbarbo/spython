from typing import Sequence
def soma(xs: Sequence[int]) -> int:
    t = 0
    for x in xs:
        t += x
    return t + xs[0] + len(xs)
print(soma([1, 2]), soma((3, 4)))
