from typing import Callable
def dobra(f: Callable[[int], int]) -> Callable[[int], int]:
    return lambda x: 2 * f(x)
@dobra
def g(x: int) -> int:
    return x + 1
print(g(1))
