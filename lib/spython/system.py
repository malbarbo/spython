import sys
import builtins
import time as _time


def sleep(ms: int) -> None:
    _time.sleep(ms / 1000.0)


# A monotonic clock, so a change of the system clock does not stop or skip
# the ticks of a world.
def now_ms() -> int:
    return int(_time.monotonic() * 1000)


from _spython_ffi import show_svg  # type: ignore[import-not-found]

# The page draws an svg instead, so it has no printer.
try:
    from _spython_display import Printer as _Printer  # type: ignore[import-not-found]
except ImportError:
    _Printer = None


def install_displayhook() -> None:
    _original_displayhook = sys.displayhook
    printer = None if _Printer is None else _Printer()

    def _displayhook(value: object) -> None:
        from spython.image import Image as _Image, _to_repl_svg

        if not isinstance(value, _Image):
            _original_displayhook(value)
            return
        if printer is None:
            try:
                svg = _to_repl_svg(value)
            except ValueError:
                # An image too large to draw prints as any other value.
                _original_displayhook(value)
                return
            show_svg(svg)
        # A terminal with no graphics, or one that a display holds, prints the
        # image as any other value, and so does an image too large to draw.
        elif not printer.show(value):
            _original_displayhook(value)
            return
        builtins._ = value  # type: ignore[attr-defined]

    sys.displayhook = _displayhook
