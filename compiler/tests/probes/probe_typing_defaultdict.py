from typing import DefaultDict
d: DefaultDict[str, int] = DefaultDict(int)
d["a"] += 1
print(d["a"])
