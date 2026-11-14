from typing import Final, Literal
N: Final = 3
def f(m: Literal["a", "b"]) -> str:
    return m
print(N, f("a"))
