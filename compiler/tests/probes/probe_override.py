from typing import override, final
class A:
    def f(self) -> int:
        return 1
@final
class B(A):
    @override
    def f(self) -> int:
        return 2
print(B().f())
