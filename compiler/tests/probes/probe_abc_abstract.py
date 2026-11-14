from abc import ABC, abstractmethod
class Forma(ABC):
    @abstractmethod
    def area(self) -> float: ...
class Q(Forma):
    def area(self) -> float:
        return 1.0
print(Q().area())
