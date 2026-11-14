class A:
    def f(self) -> int:
        return 1
class B(A):
    def f(self) -> int:
        return super(B, self).f() + 1
print(B().f())
