from collections.abc import Callable

from spython.image import Image, _canvas_size, to_svg
from spython.system import get_key_event, now_ms, show_svg, sleep

try:
    from _spython_display import Display  # type: ignore[import-not-found]
except ImportError:
    Display = None

KEYPRESS: int = 0
KEYDOWN: int = 1
KEYUP: int = 2

MS_PER_SECOND: int = 1000
KEY_POLL_PERIOD_MS: int = 10
DEFAULT_TICK_RATE: int = 28
MIN_TICK_RATE: int = 1
MAX_TICK_RATE: int = MS_PER_SECOND // KEY_POLL_PERIOD_MS

# The kinds of event that `Display.wait_event` returns after the three kinds
# of key.
VSYNC: int = 3
CLOSE: int = 4

TITLE: str = "spython"


class World[S]:
    def __init__(self, state: S, to_image: Callable[[S], Image]) -> None:
        self.state: S = state
        self.to_image: Callable[[S], Image] = to_image
        self.rate: int = DEFAULT_TICK_RATE
        self.on_tick_fn: Callable[[S], S] | None = None
        self.stop_when_fn: Callable[[S], bool] | None = None
        self.on_key_press_fn: Callable[[S, str], S] | None = None
        self.on_key_down_fn: Callable[[S, str], S] | None = None
        self.on_key_up_fn: Callable[[S, str], S] | None = None

    def on_tick(self, handler: Callable[[S], S]) -> "World[S]":
        self.on_tick_fn = handler
        return self

    def tick_rate(self, rate: int) -> "World[S]":
        self.rate = max(MIN_TICK_RATE, min(MAX_TICK_RATE, rate))
        return self

    def stop_when(self, handler: Callable[[S], bool]) -> "World[S]":
        self.stop_when_fn = handler
        return self

    def on_key_press(self, handler: Callable[[S, str], S]) -> "World[S]":
        self.on_key_press_fn = handler
        return self

    def on_key_down(self, handler: Callable[[S, str], S]) -> "World[S]":
        self.on_key_down_fn = handler
        return self

    def on_key_up(self, handler: Callable[[S, str], S]) -> "World[S]":
        self.on_key_up_fn = handler
        return self

    def run(self) -> None:
        if Display is not None:
            self._run_on_display()
            return
        self._show()
        next_tick_at: int = now_ms() + MS_PER_SECOND // self.rate
        while True:
            now: int = now_ms()
            if now >= next_tick_at:
                if self.on_tick_fn is not None:
                    self.state = self.on_tick_fn(self.state)
                    self._show()
                next_tick_at = _next_deadline(
                    next_tick_at, now, MS_PER_SECOND // self.rate
                )

            event: tuple[int, str, bool, bool, bool, bool, bool] | None = (
                get_key_event()
            )
            # event[2..6]: alt, ctrl, shift, meta, repeat
            if event is not None and self._on_key(event[0], event[1]):
                self._show()

            if self.stop_when_fn is not None and self.stop_when_fn(self.state):
                self._show()
                return

            wait_ms: int = min(next_tick_at - now_ms(), KEY_POLL_PERIOD_MS)
            if wait_ms > 0:
                sleep(wait_ms)

    def _show(self) -> None:
        show_svg(to_svg(self.to_image(self.state)))

    def _run_on_display(self) -> None:
        w, h = _canvas_size(self.to_image(self.state))
        display = Display(TITLE, w, h)
        try:
            self._loop(display)
        finally:
            display.close()

    # The display sends a Vsync when it can show a frame, and a tick falls on
    # the first Vsync after its time. Once stop_when holds, the last image
    # goes out and the run ends. The terminal keeps it on screen.
    def _loop(self, display: object) -> None:
        next_tick_at: int = now_ms() + MS_PER_SECOND // self.rate
        changed: bool = True
        after_down: bool = False
        while True:
            event: tuple[int, str, bool, bool, bool, bool, bool] = display.wait_event()  # type: ignore[attr-defined]
            kind: int = event[0]
            if kind == CLOSE:
                return
            if kind == VSYNC:
                now: int = now_ms()
                if now >= next_tick_at:
                    if self.on_tick_fn is not None:
                        self.state = self.on_tick_fn(self.state)
                        changed = True
                    next_tick_at = _next_deadline(
                        next_tick_at, now, MS_PER_SECOND // self.rate
                    )
                if changed:
                    _present(display, self.to_image(self.state))
                    changed = False
            else:
                # A terminal that does not see a key go down sends a press
                # alone, and the press stands for the down too.
                if kind == KEYPRESS and not after_down:
                    changed = self._on_key(KEYDOWN, event[1]) or changed
                after_down = kind == KEYDOWN
                changed = self._on_key(kind, event[1]) or changed
            if self.stop_when_fn is not None and self.stop_when_fn(self.state):
                _present(display, self.to_image(self.state))
                return

    def _on_key(self, kind: int, key: str) -> bool:
        if self.on_key_down_fn is not None and kind == KEYDOWN:
            self.state = self.on_key_down_fn(self.state, key)
            return True
        if self.on_key_press_fn is not None and kind == KEYPRESS:
            self.state = self.on_key_press_fn(self.state, key)
            return True
        if self.on_key_up_fn is not None and kind == KEYUP:
            self.state = self.on_key_up_fn(self.state, key)
            return True
        return False


# Schedule from the deadline (not from now) to absorb minor overruns without
# drift. If we overran by more than a full period, snap forward so we don't
# burn ticks catching up.
def _next_deadline(deadline: int, now: int, period_ms: int) -> int:
    deadline += period_ms
    if deadline <= now:
        return now + period_ms
    return deadline


def _present(display: object, image: Image) -> None:
    w, h = _canvas_size(image)
    display.present(image, w, h)  # type: ignore[attr-defined]


def animate(create_image: Callable[[int], Image]) -> None:
    if Display is not None:
        _animate_on_display(create_image)
        return
    period_ms: int = MS_PER_SECOND // DEFAULT_TICK_RATE
    next_frame_at: int = now_ms()
    frame: int = 0
    while True:
        show_svg(to_svg(create_image(frame)))
        next_frame_at += period_ms
        wait_ms: int = next_frame_at - now_ms()
        if wait_ms > 0:
            sleep(wait_ms)
        else:
            next_frame_at = now_ms()
        frame += 1


def _animate_on_display(create_image: Callable[[int], Image]) -> None:
    w, h = _canvas_size(create_image(0))
    display = Display(TITLE, w, h)
    try:
        period_ms: int = MS_PER_SECOND // DEFAULT_TICK_RATE
        next_frame_at: int = now_ms()
        frame: int = 0
        while True:
            event: tuple[int, str, bool, bool, bool, bool, bool] = display.wait_event()
            if event[0] == CLOSE:
                return
            now: int = now_ms()
            if event[0] == VSYNC and now >= next_frame_at:
                _present(display, create_image(frame))
                frame += 1
                next_frame_at = _next_deadline(next_frame_at, now, period_ms)
    finally:
        display.close()
