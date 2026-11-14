from typing import Iterator
def a() -> Iterator[int]:
    yield 1
    yield from [2, 3]
print(list(a()))
