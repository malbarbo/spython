from typing import NewType
Id = NewType("Id", int)
def f(i: Id) -> int:
    return i + 1
print(f(Id(3)))
