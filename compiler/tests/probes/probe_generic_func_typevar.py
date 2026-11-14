from typing import TypeVar
T = TypeVar("T")
def primeiro(xs: list[T]) -> T:
    return xs[0]
print(primeiro([1, 2]), primeiro(["a"]))
