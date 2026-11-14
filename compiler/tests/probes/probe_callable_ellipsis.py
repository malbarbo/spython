from typing import Callable
def ap(f: Callable[..., int]) -> int:
    return f(1)
print(ap(lambda x: x))
