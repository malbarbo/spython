from dataclasses import dataclass, field
@dataclass
class C:
    xs: list[int] = field(default_factory=list)
c = C()
c.xs.append(1)
print(c)
