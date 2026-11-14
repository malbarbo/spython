xs = ["aa", "b"]
ys: list[object] = []
ys.append(max(xs, key=lambda s: len(s)))
print(len(ys))
