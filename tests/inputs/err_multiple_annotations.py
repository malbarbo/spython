# Test file for spython annotation checking


def add(x, y):  # Missing parameter and return annotations
    return x + y


def greet(name: str):  # Missing return annotation
    print(f"Hello, {name}!")


class Point:
    x = 0  # Missing annotation
    y = 0  # Missing annotation

    def __init__(self, x: int, y: int):  # Missing return annotation
        self.x = x
        self.y = y

    def distance(self):  # Missing self is OK, but missing return annotation
        return (self.x**2 + self.y**2) ** 0.5


def good_function(a: int, b: str) -> None:  # All annotations present - should be OK
    print(a, b)


class GoodClass:
    value: int = 42  # Has annotation - should be OK

    def method(self, param: str) -> str:  # All annotations - should be OK
        return param
