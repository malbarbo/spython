import enum
from enum import Enum as E, IntEnum, StrEnum


class Color(enum.Enum):
    RED = 1
    GREEN = 2


class Base(E):
    pass


class Size(Base):
    SMALL = 1


class Level(IntEnum):
    LOW = 1


class Name(StrEnum):
    ANA = "ana"


print(Color.RED.name, Size.SMALL.name, Level.LOW.value, Name.ANA.value)
