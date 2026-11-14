t = str.maketrans("ab", "xy")
print("abc".translate(t), "a\tb".isprintable())
