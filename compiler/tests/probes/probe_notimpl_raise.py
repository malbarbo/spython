class F:
    def area(self) -> float:
        raise NotImplementedError
class Q(F):
    def area(self) -> float:
        return 1.0
xs: list[F] = [Q()]
print(xs[0].area())
