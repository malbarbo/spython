from enum import Enum, IntEnum
class Cor(Enum):
    A = 1
    B = 2
class N(IntEnum):
    X = 1
print(len(Cor), Cor(1), Cor["B"], N.X + 1)
