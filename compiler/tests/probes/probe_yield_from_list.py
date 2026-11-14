def g() -> list[int]:
    return [1]
from typing import Iterator
def h() -> Iterator[int]:
    yield 1
    yield from [2, 3]
print(list(h()))
