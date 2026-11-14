from dataclasses import dataclass
@dataclass
class C:
    xs: list[int]
c = C([])
c.xs.append(2)
print(c)
