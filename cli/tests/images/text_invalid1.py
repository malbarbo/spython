from typing import Callable

from spython.image import Font, Image, height, text


def show(make: Callable[[], Image | Font]) -> None:
    try:
        made = make()
        print(height(made) if isinstance(made, Image) else "ok")
    except ValueError as e:
        print(e)


show(lambda: text("abc", size=0))
show(lambda: text("abc", size=-12))
show(lambda: Font(size=-1))
show(lambda: text("abc", font=Font(size=-3.5)))
show(lambda: text("a\ud800b"))
show(lambda: Font(family="\ud800"))
show(lambda: text("a", size=1e39))
