class A:
    def a(self) -> int:
        return 1
class B:
    def b(self) -> int:
        return 2
class C(A, B):
    pass
c = C()
print(c.a(), c.b())
