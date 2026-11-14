class Forma:
    def area(self) -> float:
        return 0.0
class Q(Forma):
    lado: float
    def __init__(self, lado: float) -> None:
        self.lado = lado
    def area(self) -> float:
        return self.lado ** 2
class C(Forma):
    r: float
    def __init__(self, r: float) -> None:
        self.r = r
    def area(self) -> float:
        return 3.14 * self.r ** 2
fs: list[Forma] = [Q(2.0), C(1.0)]
print([f.area() for f in fs])
