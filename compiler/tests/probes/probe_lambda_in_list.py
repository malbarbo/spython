from typing import Callable
fs: list[Callable[[int], int]] = [lambda x: x + 1, lambda x: x * 2]
print([f(3) for f in fs])
