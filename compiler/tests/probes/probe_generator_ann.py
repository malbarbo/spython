from typing import Generator
def h() -> Generator[int, None, None]:
    yield 1
print(list(h()))
