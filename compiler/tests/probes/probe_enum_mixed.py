from enum import Enum
class C(Enum):
    A = 1
    B = "b"
print(C.B.value)
