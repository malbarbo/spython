from dataclasses import dataclass
class Base:
    def ola(self) -> str:
        return "ola"
@dataclass
class D(Base):
    x: int
print(D(1).ola(), D(1))
