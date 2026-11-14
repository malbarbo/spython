from typing import NoReturn, Never
def falha(m: str) -> NoReturn:
    assert False, m
def f(x: int) -> int:
    if x > 0:
        return x
    falha("neg")
print(f(1))
