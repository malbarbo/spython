from dataclasses import dataclass, field
@dataclass
class T:
    notas: list[float] = field(default_factory=list)
print(T())
