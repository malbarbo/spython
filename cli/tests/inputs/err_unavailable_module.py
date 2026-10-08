from __future__ import annotations

import os.path
from collections.abc import Sequence
from typing import Any

from spython.image import circle


def first(items: Sequence[Any]) -> Any:
    import re

    return items[0]


print(circle, first([1]), os.path)
