class Q:
    def area(self) -> float:
        return 2.0
def total(xs: list[Q]) -> float:
    return sum(x.area() for x in xs)
print(total([Q()]))
