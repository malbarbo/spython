import sys
import builtins
import time as _time


def sleep(ms: int) -> None:
    _time.sleep(ms / 1000.0)


def now_ms() -> int:
    return int(_time.time() * 1000)


from _spython_ffi import show_svg  # type: ignore[import-not-found]

# Opens a printer of images in the terminal, or gives None when the terminal
# shows no graphics.
try:
    from _spython_display import open_printer as _open_printer  # type: ignore[import-not-found]
except ImportError:
    _open_printer = None


def install_displayhook() -> None:
    _original_displayhook = sys.displayhook
    # The printer opens at the first image, since the probe of the terminal
    # waits for its answer.
    printer: object = None
    opened: bool = False

    def _displayhook(value: object) -> None:
        nonlocal printer, opened
        from spython.image import Image as _Image, _to_repl_svg

        if not isinstance(value, _Image):
            _original_displayhook(value)
            return
        if _open_printer is None:
            try:
                svg = _to_repl_svg(value)
            except ValueError:
                # An image too large to draw prints as any other value.
                _original_displayhook(value)
                return
            show_svg(svg)
        else:
            if not opened:
                printer = _open_printer()
                opened = True
            # A terminal with no graphics, or one that a display holds, prints
            # the image as any other value, and so does an image too large to
            # draw.
            if printer is None or not printer.show(value):  # type: ignore[attr-defined]
                _original_displayhook(value)
                return
        builtins._ = value  # type: ignore[attr-defined]

    sys.displayhook = _displayhook
