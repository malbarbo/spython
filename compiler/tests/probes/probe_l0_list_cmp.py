def f(a: str, b: str) -> bool:
    return a.split() < b.split()
print(f("a b", "a c"))
