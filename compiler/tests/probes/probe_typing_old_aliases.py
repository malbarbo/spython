from typing import List, Dict, Optional, Union
def f(xs: List[int], d: Dict[str, int], o: Optional[int], u: Union[int, str]) -> int:
    return len(xs) + len(d)
print(f([1], {"a": 1}, None, 1))
