from typing import Iterable
def h() -> Iterable[int]:
    yield 1
print(list(h()))
