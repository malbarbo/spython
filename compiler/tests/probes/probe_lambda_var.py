from typing import Callable
f: Callable[[int], int] = lambda x: x + 1
g = lambda: 3
print(f(1), g())
