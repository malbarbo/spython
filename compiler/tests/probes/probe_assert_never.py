from enum import Enum
from typing import assert_never
class C(Enum):
    A = 1
    B = 2
def f(c: C) -> str:
    match c:
        case C.A:
            return "a"
        case C.B:
            return "b"
        case _:
            assert_never(c)
print(f(C.A))
