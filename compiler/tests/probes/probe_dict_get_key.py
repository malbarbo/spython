d = {"a": 3, "b": 5}
print(max(d, key=d.get), max(d, key=lambda k: d[k]))
