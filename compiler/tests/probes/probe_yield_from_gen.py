from typing import Iterator
def a() -> Iterator[int]:
    yield 1
def h() -> Iterator[int]:
    yield from a()
print(list(h()))
