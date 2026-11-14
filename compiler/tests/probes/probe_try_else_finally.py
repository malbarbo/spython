def f(n: int) -> None:
    try:
        x = 10 // n
    except ZeroDivisionError:
        print("zero")
    else:
        print("ok", x)
    finally:
        print("fim")
f(2)
f(0)
