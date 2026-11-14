class Ctx:
    def __enter__(self) -> "Ctx":
        print("enter")
        return self
    def __exit__(self, a: object, b: object, c: object) -> None:
        print("exit")
with Ctx() as c:
    print("dentro")
