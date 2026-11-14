from typing import Callable
def aplica(f: Callable[[int], int], x: int) -> int:
    return f(x)
def dobro(x: int) -> int:
    return 2 * x
print(aplica(dobro, 3), aplica(lambda x: x + 1, 3))
