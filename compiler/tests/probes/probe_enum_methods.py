from enum import Enum, auto
class Dir(Enum):
    N = auto()
    S = auto()
    def oposta(self) -> "Dir":
        if self == Dir.N:
            return Dir.S
        return Dir.N
print(Dir.N.oposta())
