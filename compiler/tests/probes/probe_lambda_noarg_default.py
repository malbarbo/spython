from typing import Callable
f: Callable[[int], int] = lambda x, k=2: x * k
print(f(3))
