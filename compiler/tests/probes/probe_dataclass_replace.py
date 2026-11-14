from dataclasses import dataclass, replace
@dataclass
class C:
    x: int
    y: int
print(replace(C(1, 2), y=3))
