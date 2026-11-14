from enum import Enum
class C(Enum):
    A = 1
    B = 2
    def prox(self) -> "C":
        if self == C.A:
            return C.B
        return C.A
print(C.A.prox())
