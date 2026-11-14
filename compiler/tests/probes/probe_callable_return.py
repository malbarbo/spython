from typing import Callable
def soma_n(n: int) -> Callable[[int], int]:
    return lambda x: x + n
print(soma_n(2)(3))
