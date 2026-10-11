from collections.abc import Callable

from _spython_display import Display  # type: ignore[import-not-found]

from spython.image import Image, _canvas_size
from spython.system import now_ms

KEYPRESS: int = 0
KEYDOWN: int = 1
KEYUP: int = 2

MS_PER_SECOND: int = 1000
KEY_POLL_PERIOD_MS: int = 10
DEFAULT_TICK_RATE: int = 28
MIN_TICK_RATE: int = 1
MAX_TICK_RATE: int = MS_PER_SECOND // KEY_POLL_PERIOD_MS

# The kind of event that `Display.wait_event` returns after the three kinds
# of key.
CLOSE: int = 3


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
        display = Display()
        try:
            self._show_on(display)
            self._loop(display)
        finally:
            display.close()

    # The world keeps its own clock, and shows its image when a tick or a key
    # changes the state. The display draws the last image at its next frame.
    # Once stop_when holds, the run ends, and the close draws the last image.
    def _loop(self, display: object) -> None:
        period_ms: int = MS_PER_SECOND // self.rate
        next_tick_at: int = now_ms() + period_ms
        while self.stop_when_fn is None or not self.stop_when_fn(self.state):
            event: tuple[int, str, bool, bool, bool, bool] | None = display.wait_event(  # type: ignore[attr-defined]
                max(next_tick_at - now_ms(), 0)
            )
            if event is not None and event[0] == CLOSE:
                return
            changed: bool = False
            now: int = now_ms()
            if now >= next_tick_at:
                if self.on_tick_fn is not None:
                    self.state = self.on_tick_fn(self.state)
                    changed = True
                next_tick_at = _next_deadline(next_tick_at, now, period_ms)
            if event is not None and self._on_key(event[0], event[1]):
                changed = True
            if changed:
                self._show_on(display)

    def _show_on(self, display: object) -> None:
        _show(display, self.to_image(self.state))

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


def _show(display: object, image: Image) -> None:
    w, h = _canvas_size(image)
    display.show(image, w, h)  # type: ignore[attr-defined]


def animate(create_image: Callable[[int], Image]) -> None:
    display = Display()
    try:
        period_ms: int = MS_PER_SECOND // DEFAULT_TICK_RATE
        frame: int = 0
        while True:
            _show(display, create_image(frame))
            if _closed_before(display, now_ms() + period_ms):
                return
            frame += 1
    finally:
        display.close()


# Returns True if the display closes before `deadline`, False otherwise. The
# keys that arrive meanwhile go unused.
def _closed_before(display: object, deadline: int) -> bool:
    while True:
        event: tuple[int, str, bool, bool, bool, bool] | None = display.wait_event(  # type: ignore[attr-defined]
            max(deadline - now_ms(), 0)
        )
        if event is None:
            return False
        if event[0] == CLOSE:
            return True
