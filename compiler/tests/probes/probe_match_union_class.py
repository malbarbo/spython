from dataclasses import dataclass
@dataclass
class A:
    x: int
@dataclass
class B:
    y: str
def f(v: A | B) -> str:
    match v:
        case A(x=x):
            return str(x)
        case B(y=y):
            return y
print(f(A(1)), f(B("b")))
