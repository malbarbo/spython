class F:
    def __call__(self, x: int) -> int:
        return x + 1
print(F()(1))
