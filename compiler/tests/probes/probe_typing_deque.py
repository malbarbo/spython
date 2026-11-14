from typing import Deque
q: Deque[int] = Deque()
q.appendleft(1)
print(q.popleft())
