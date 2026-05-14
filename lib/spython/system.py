import sys
import builtins


# Text measurement and asset loading are flat FFI in both native and wasm.
try:
    from _spython_ffi import (
        text_width,
        text_height,
        text_x_offset,
        text_y_offset,
        load_bitmap,
    )  # type: ignore[import-not-found]
except ImportError:

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


# `Frontend` is the typed pyclass that owns a `simage::frontend::Frontend`.
# Only the native runtime ships it; CPython and wasm fall back to a Python
# shim that delegates to whichever flat FFI is available (wasm) or returns
# inert defaults (plain CPython).
try:
    from _frontend import Frontend  # type: ignore[import-not-found]
except ImportError:
    try:
        from _spython_ffi import (  # type: ignore[import-not-found]
            show_svg as _ffi_show_svg,
            kitty_supported as _ffi_kitty_supported,
            sixel_supported as _ffi_sixel_supported,
            text_blocks_supported as _ffi_text_blocks_supported,
            wait_event as _ffi_wait_event,
            enter_animation as _ffi_enter_animation,
            exit_animation as _ffi_exit_animation,
        )

        class Frontend:  # type: ignore[no-redef]
            """Wasm fallback: a thin shim over the flat `_spython_ffi` API."""

            def enter(self) -> None:
                _ffi_enter_animation()

            def exit(self) -> None:
                _ffi_exit_animation()

            def wait_event(
                self, deadline_ms: int | None = None
            ) -> tuple[int, str, bool, bool, bool, bool, bool] | None:
                if deadline_ms is None:
                    return _ffi_wait_event()
                return _ffi_wait_event(deadline_ms)

            def present(self, dl: "DrawList") -> None:
                raise RuntimeError("Frontend.present is not available on wasm")

            def show_svg(self, svg: str) -> None:
                _ffi_show_svg(svg)

            def kitty_supported(self) -> bool:
                return _ffi_kitty_supported()

            def sixel_supported(self) -> bool:
                return _ffi_sixel_supported()

            def text_blocks_supported(self) -> bool:
                return _ffi_text_blocks_supported()
    except ImportError:

        class Frontend:  # type: ignore[no-redef]
            """Plain-CPython fallback: no host, SVG goes to stdout."""

            def enter(self) -> None:
                return None

            def exit(self) -> None:
                return None

            def wait_event(
                self, deadline_ms: int | None = None
            ) -> tuple[int, str, bool, bool, bool, bool, bool] | None:
                return None

            def present(self, dl: "DrawList") -> None:
                raise RuntimeError(
                    "Frontend.present is not available without the spython runtime"
                )

            def show_svg(self, svg: str) -> None:
                print(svg)

            def kitty_supported(self) -> bool:
                return False

            def sixel_supported(self) -> bool:
                return False

            def text_blocks_supported(self) -> bool:
                return False


def frontend() -> Frontend:
    """Construct a fresh `Frontend`. There is no Python-level singleton —
    long-running call sites (`World.run`) build one and keep it for the
    duration of the loop; one-shot callers (REPL displayhook, `show()`)
    build a transient instance per call. Frontends are cheap to construct
    in either mode (a window opens only on `enter()`)."""
    return Frontend()


# `_drawlist` is the native pyclass module that backs the typed draw-list
# pipeline (terminal renderer + PDF output). It only exists in the spython
# runtime; on plain CPython or WASM we fall back to a stub that raises.
try:
    from _drawlist import DrawList  # type: ignore[import-not-found]
except ImportError:

    class DrawList:  # type: ignore[no-redef]
        def __init__(self, width: float, height: float) -> None:
            raise RuntimeError("DrawList is not available without the spython runtime")


def render(fr: Frontend, value: "_Image") -> None:  # type: ignore[name-defined] # noqa: F821
    """Render an `Image` through the given `Frontend`. Picks the raster
    path (typed DrawList) when the frontend advertises support, otherwise
    falls back to SVG text."""
    from spython.image import (
        Image as _Image,
        to_svg as _to_svg,
        to_drawlist as _to_drawlist,
    )

    if not isinstance(value, _Image):
        return
    if fr.kitty_supported() or fr.sixel_supported() or fr.text_blocks_supported():
        fr.present(_to_drawlist(value))
    else:
        fr.show_svg(_to_svg(value))


def show(value: "_Image") -> None:  # type: ignore[name-defined] # noqa: F821
    """One-shot display for REPL / displayhook callers. Builds a transient
    `Frontend` and routes through `render`. Loops that draw many frames
    should hold onto a `Frontend` themselves (see `World.run`)."""
    render(frontend(), value)


def show_svg(svg: str) -> None:
    """Display a raw SVG string through a transient `Frontend`."""
    frontend().show_svg(svg)


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
