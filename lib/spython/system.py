import sys
import builtins


try:
    from _spython_ffi import (
        show_svg,
        kitty_supported,
        sixel_supported,
        text_blocks_supported,
        wait_event,
        text_width,
        text_height,
        text_x_offset,
        text_y_offset,
        load_bitmap,
        enter_animation,
        exit_animation,
    )  # type: ignore[import-not-found]
except ImportError:

    def show_svg(svg: str) -> None:
        print(svg)

    def kitty_supported() -> bool:
        return False

    def sixel_supported() -> bool:
        return False

    def text_blocks_supported() -> bool:
        return False

    def wait_event(
        deadline_ms: int | None = None,
    ) -> tuple[int, str, bool, bool, bool, bool, bool] | None:
        return None

    def text_width(text: str, font_css: str, size: int) -> float:
        return len(text) * size * 0.6

    def text_height(text: str, font_css: str, size: int) -> float:
        return float(size)

    def text_x_offset(text: str, font_css: str, size: int) -> float:
        return 0.0

    def text_y_offset(text: str, font_css: str, size: int) -> float:
        return 0.0

    def load_bitmap(path: str) -> tuple[float, float, str]:
        raise RuntimeError("load_bitmap is not available in native mode")

    def enter_animation() -> None:
        return None

    def exit_animation() -> None:
        return None


# `_drawlist` is the native pyclass module that backs the typed draw-list
# pipeline (terminal renderer + PDF output). It only exists in the spython
# runtime; on plain CPython or WASM we fall back to a stub that raises.
try:
    from _drawlist import DrawList  # type: ignore[import-not-found]
except ImportError:

    class DrawList:  # type: ignore[no-redef]
        def __init__(self, width: float, height: float) -> None:
            raise RuntimeError("DrawList is not available without the spython runtime")


def show(value: "_Image") -> None:  # type: ignore[name-defined] # noqa: F821
    """Display an Image. Routes through the Kitty / Sixel / half-blocks
    backend when the host advertises support, otherwise falls back to
    printing SVG."""
    from spython.image import (
        Image as _Image,
        to_svg as _to_svg,
        to_drawlist as _to_drawlist,
    )

    if not isinstance(value, _Image):
        return
    if kitty_supported() or sixel_supported() or text_blocks_supported():
        _to_drawlist(value).show()
    else:
        show_svg(_to_svg(value))


def install_displayhook() -> None:
    _original_displayhook = sys.displayhook

    def _displayhook(value: object) -> None:
        from spython.image import Image as _Image

        if isinstance(value, _Image):
            show(value)
            builtins._ = value  # type: ignore[attr-defined]
        else:
            _original_displayhook(value)

    sys.displayhook = _displayhook
