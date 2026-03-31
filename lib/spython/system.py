import sys
import builtins

try:
    from _spython_ffi import (
        show_svg,
        get_key_event,
        text_width,
        text_height,
        text_x_offset,
        text_y_offset,
        load_bitmap,
    )  # type: ignore[import-not-found]
except ImportError:

    def show_svg(svg: str) -> None:
        print(svg)

    def get_key_event() -> tuple[int, str, bool, bool, bool, bool, bool] | None:
        return None

    def text_width(text: str, font_css: str) -> float:
        size: float = 14.0
        parts: list[str] = font_css.split("px")
        if len(parts) >= 2:
            num: str = parts[0].split()[-1]
            try:
                size = float(num)
            except ValueError:
                pass
        return len(text) * size * 0.6

    def text_height(text: str, font_css: str) -> float:
        size: float = 14.0
        parts: list[str] = font_css.split("px")
        if len(parts) >= 2:
            num: str = parts[0].split()[-1]
            try:
                size = float(num)
            except ValueError:
                pass
        return size

    def text_x_offset(text: str, font_css: str) -> float:
        return 0.0

    def text_y_offset(text: str, font_css: str) -> float:
        return 0.0

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
