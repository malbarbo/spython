from enum import Enum, auto
class C(Enum):
    A = auto()
    B = auto()
print(C.B.value, C.A)
