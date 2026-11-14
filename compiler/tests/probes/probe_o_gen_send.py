from typing import Generator
def g() -> Generator[int, int, None]:
    x = yield 1
    print(x)
it = g()
next(it)
try:
    it.send(5)
except StopIteration:
    pass
