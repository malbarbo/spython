from dataclasses import dataclass
@dataclass
class A:
    x: int
@dataclass
class B:
    y: str
def f(v: A | B) -> str:
    if isinstance(v, A):
        return str(v.x)
    return v.y
print(f(A(1)), f(B("b")))
