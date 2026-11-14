from typing import Mapping
def total(d: Mapping[str, int]) -> int:
    return sum(d.values())
print(total({"a": 1}))
