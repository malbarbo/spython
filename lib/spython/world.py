from collections.abc import Callable

from _spython_display import (  # type: ignore[import-not-found]
    CLOSE as _CLOSE,
    KEYDOWN as _KEYDOWN,
    KEYPRESS as _KEYPRESS,
    KEYUP as _KEYUP,
    close as _close,
    show as _show,
    wait_event as _wait_event,
)

from spython.image import Image
from spython.system import now_ms

MS_PER_SECOND: int = 1000
KEY_POLL_PERIOD_MS: int = 10
DEFAULT_TICK_RATE: int = 28
MIN_TICK_RATE: int = 1
MAX_TICK_RATE: int = MS_PER_SECOND // KEY_POLL_PERIOD_MS


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
        try:
            _show(self.to_image(self.state))
            self._loop()
        finally:
            _close()

    # The world keeps its own clock, and shows its image when a tick or a key
    # changes the state. The display draws the last image at its next frame.
    # Once stop_when holds, the run ends, and the close draws the last image.
    def _loop(self) -> None:
        period_ms: int = MS_PER_SECOND // self.rate
        next_tick_at: int = now_ms() + period_ms
        while self.stop_when_fn is None or not self.stop_when_fn(self.state):
            event: tuple[int, str] | None = _wait_event(max(next_tick_at - now_ms(), 0))
            if event is not None and event[0] == _CLOSE:
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
                _show(self.to_image(self.state))

    def _on_key(self, kind: int, key: str) -> bool:
        if self.on_key_down_fn is not None and kind == _KEYDOWN:
            self.state = self.on_key_down_fn(self.state, key)
            return True
        if self.on_key_press_fn is not None and kind == _KEYPRESS:
            self.state = self.on_key_press_fn(self.state, key)
            return True
        if self.on_key_up_fn is not None and kind == _KEYUP:
            self.state = self.on_key_up_fn(self.state, key)
            return True
        return False


# The next deadline counts from the last one, so a late tick does not delay
# the ticks after it. When a tick is a whole period late, the next deadline is
# one period from now, and the ticks it missed do not run.
def _next_deadline(deadline: int, now: int, period_ms: int) -> int:
    deadline += period_ms
    if deadline <= now:
        return now + period_ms
    return deadline


def animate(create_image: Callable[[int], Image]) -> None:
    try:
        period_ms: int = MS_PER_SECOND // DEFAULT_TICK_RATE
        frame: int = 0
        while True:
            _show(create_image(frame))
            if _closed_before(now_ms() + period_ms):
                return
            frame += 1
    finally:
        _close()


# Returns True if the display closes before `deadline`, False otherwise. The
# keys that arrive meanwhile go unused.
def _closed_before(deadline: int) -> bool:
    while True:
        event: tuple[int, str] | None = _wait_event(max(deadline - now_ms(), 0))
        if event is None:
            return False
        if event[0] == _CLOSE:
            return True
