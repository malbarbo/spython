from typing import Optional, Union, Final, Literal
X: Final = 3
def f(x: Optional[int], m: Literal["a", "b"]) -> Union[int, str]:
    return m
print(f(1, "a"), X)
