total = 0
def f() -> None:
    def g() -> None:
        global total
        total += 1
    g()
f()
print(total)
