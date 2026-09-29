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
        bitmap_size,
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

    def load_bitmap(path: str) -> str:
        raise RuntimeError("load_bitmap is not available in native mode")

    def bitmap_size(data_uri: str) -> tuple[float, float]:
        raise RuntimeError("bitmap_size is not available in native mode")


# Opens a printer of images in the terminal, or gives None when the terminal
# shows no graphics.
try:
    from _spython_display import open_printer as _open_printer  # type: ignore[import-not-found]
except ImportError:
    _open_printer = None

# The image of a data URI, which a bitmap keeps so the display draws it
# with no decode, or None where nothing displays, as on the page.
try:
    from _spython_display import load_image  # type: ignore[import-not-found]
except ImportError:

    def load_image(data_uri: str) -> object:
        return None


def install_displayhook() -> None:
    _original_displayhook = sys.displayhook
    # The printer opens at the first image, since the probe of the terminal
    # waits for its answer.
    printer: object = None
    opened: bool = False

    def _displayhook(value: object) -> None:
        nonlocal printer, opened
        from spython.image import Image as _Image, _canvas_size, to_svg as _to_svg

        if not isinstance(value, _Image):
            _original_displayhook(value)
            return
        if _open_printer is None:
            show_svg(_to_svg(value))
        else:
            if not opened:
                printer = _open_printer()
                opened = True
            # A terminal with no graphics, or one that a display holds,
            # prints the image as any other value.
            if printer is None or not printer.show(value, *_canvas_size(value)):  # type: ignore[attr-defined]
                _original_displayhook(value)
                return
        builtins._ = value  # type: ignore[attr-defined]

    sys.displayhook = _displayhook
