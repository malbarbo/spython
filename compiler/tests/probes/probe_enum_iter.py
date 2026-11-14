from enum import Enum, auto
class Cor(Enum):
    A = auto()
    B = auto()
for c in Cor:
    print(c, c.name, c.value)
print(len(Cor), Cor(1), Cor["B"])
