class C(object):
    pass
class D():
    pass
print(isinstance(C(), object), isinstance(D(), D))
