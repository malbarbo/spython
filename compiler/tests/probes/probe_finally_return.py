def f() -> int:
    try:
        return 1
    finally:
        print("finally")
print(f())
