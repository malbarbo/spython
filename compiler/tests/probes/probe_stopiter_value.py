from typing import Generator
def g() -> Generator[int, None, str]:
    yield 1
    return "fim"
it = g()
print(next(it))
try:
    next(it)
except StopIteration as e:
    print(e.value)
