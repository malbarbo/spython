nomes = ["b", "a"]
notas = [1, 2]
print(sorted(zip(nomes, notas), key=lambda t: t[1]))
