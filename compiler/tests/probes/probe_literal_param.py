from typing import Literal
def f(d: Literal["N", "S"]) -> int:
    if d == "N":
        return 1
    return 2
print(f("S"))
