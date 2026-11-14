from dataclasses import dataclass
@dataclass
class Animal:
    nome: str
@dataclass
class Cao(Animal):
    raca: str
print(Cao("rex", "vira"))
