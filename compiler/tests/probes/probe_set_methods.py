s = {1, 2}
s.update([3])
print(s.union({4}), s.issubset({1, 2, 3, 4}), s.intersection({1}))
