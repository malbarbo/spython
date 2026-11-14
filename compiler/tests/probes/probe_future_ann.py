from __future__ import annotations
from dataclasses import dataclass
@dataclass
class No:
    v: int
    prox: No | None
print(No(1, No(2, None)).prox)
