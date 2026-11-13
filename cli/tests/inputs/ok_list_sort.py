def size(word: str) -> int:
    return len(word)


xs = [3, 1, 2]
xs.sort()
print(xs)
words = ["ccc", "a", "bb"]
words.sort(key=size, reverse=True)
print(words)
words.sort(key=lambda word: word[-1])
print(words)
