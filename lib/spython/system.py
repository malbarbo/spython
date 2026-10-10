import sys
import builtins
import time as _time


def sleep(ms: int) -> None:
    _time.sleep(ms / 1000.0)


def now_ms() -> int:
    return int(_time.time() * 1000)


from _spython_ffi import get_key_event, show_svg  # type: ignore[import-not-found]

# Opens a printer of images in the terminal, or gives None when the terminal
# shows no graphics.
try:
    from _spython_display import open_printer as _open_printer  # type: ignore[import-not-found]
except ImportError:
    _open_printer = None

from _spython_scene import load_bitmap, load_image, text_metrics, to_svg as scene_svg  # type: ignore[import-not-found]


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
