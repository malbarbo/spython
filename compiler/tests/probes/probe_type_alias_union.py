from dataclasses import dataclass
@dataclass
class A:
    x: int
@dataclass
class B:
    y: str
type Forma = A | B
def f(v: Forma) -> str:
    if isinstance(v, A):
        return str(v.x)
    return v.y
print(f(A(1)))
