def f() -> None:
    xs = [1]
    ys = xs
    zs = [1]
    print(xs is ys, xs is zs, xs is not zs)
f()
