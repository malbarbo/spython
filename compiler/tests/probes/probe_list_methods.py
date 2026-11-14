xs = [3, 1, 2]
xs.insert(0, 9)
xs.remove(1)
xs.extend([5])
print(xs, xs.count(9), xs.index(2), xs.pop(), xs.pop(0), xs)
ys = xs.copy()
ys.reverse()
print(ys)
