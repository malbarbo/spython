from enum import Enum, unique
@unique
class C(Enum):
    A = 1
print(C.A)
