from typing import Iterator
def conta(n: int) -> Iterator[int]:
    i = 0
    while i < n:
        yield i
        i += 1
for x in conta(3):
    print(x)
print(list(conta(2)))
