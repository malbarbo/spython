from typing import Callable
fat: Callable[[int], int] = lambda n: 1 if n == 0 else n * fat(n - 1)
print(fat(5))
