from dataclasses import dataclass
@dataclass
class V:
    x: float
    y: float
    def __add__(self, o: "V") -> "V":
        return V(self.x + o.x, self.y + o.y)
print(V(1.0, 2.0) + V(3.0, 4.0))
