from typing import TypedDict
class P(TypedDict):
    x: int
p: P = {"x": 1}
print(p["x"])
