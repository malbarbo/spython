from typing import cast
x: object = "a"
y = cast(int, x)
print(y + 1)
