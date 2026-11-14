from enum import Enum
class C(Enum):
    A = 1
    def __str__(self) -> str:
        return "cor " + self.name
print(C.A)
