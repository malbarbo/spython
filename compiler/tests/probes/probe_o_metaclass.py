class M(type):
    pass
class C(metaclass=M):
    pass
print(C)
