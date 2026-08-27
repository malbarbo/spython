import sys
import builtins
import time as _time


def sleep(ms: int) -> None:
    _time.sleep(ms / 1000.0)


def now_ms() -> int:
    return int(_time.time() * 1000)


try:
    from _spython_ffi import (
        show_svg,
        get_key_event,
        text_metrics,
        load_bitmap,
    )  # type: ignore[import-not-found]
except ImportError:

    def show_svg(svg: str) -> None:
        print(svg)

    def get_key_event() -> tuple[int, str, bool, bool, bool, bool, bool] | None:
        return None

    # A guess from the size, since there is no font engine here. A character
    # is 0.6 of the size wide, a line is the size high, and the baseline sits
    # 0.8 of the size below the top.
    def text_metrics(text: str, font_css: str) -> tuple[float, float, float, float]:
        size: float = _font_size(font_css)
        width: float = len(text) * size * 0.6
        ascent: float = size * 0.8
        return (width, size, -width / 2.0, ascent - size / 2.0)

    # The size in pixels the css says. Without one the size is 14, what a
    # `Font` starts with.
    def _font_size(font_css: str) -> float:
        for part in font_css.split():
            if part.endswith("px"):
                try:
                    return float(part[:-2])
                except ValueError:
                    pass
        return 14.0

    def load_bitmap(path: str) -> tuple[float, float, str]:
        raise RuntimeError("load_bitmap is not available in native mode")


def install_displayhook() -> None:
    _original_displayhook = sys.displayhook

    def _displayhook(value: object) -> None:
        from spython.image import Image as _Image, to_svg as _to_svg

        if isinstance(value, _Image):
            show_svg(_to_svg(value))
            builtins._ = value  # type: ignore[attr-defined]
        else:
            _original_displayhook(value)

    sys.displayhook = _displayhook
