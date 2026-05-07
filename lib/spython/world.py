from collections.abc import Callable

from spython.image import Image
from spython.system import (
    enter_animation,
    exit_animation,
    get_key_event,
    now_ms,
    show,
    sleep,
)

KEYPRESS: int = 0
KEYDOWN: int = 1
KEYUP: int = 2

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
        enter_animation()
        try:
            self._run_loop()
        finally:
            exit_animation()

    def _run_loop(self) -> None:
        self._show()
        period_ms: int = MS_PER_SECOND // self.rate
        next_tick_at: int = now_ms() + period_ms
        while True:
            now: int = now_ms()
            if now >= next_tick_at:
                if self.on_tick_fn is not None:
                    self.state = self.on_tick_fn(self.state)
                    self._show()
                # Schedule from the deadline (not from now) to absorb minor
                # overruns without drift. If we overran by more than a full
                # period, snap forward so we don't burn ticks catching up.
                period_ms = MS_PER_SECOND // self.rate
                next_tick_at += period_ms
                if next_tick_at <= now:
                    next_tick_at = now + period_ms

            event: tuple[int, str, bool, bool, bool, bool, bool] | None = (
                get_key_event()
            )
            if event is not None:
                event_type: int = event[0]
                key: str = event[1]
                # event[2..6]: alt, ctrl, shift, meta, repeat
                handled: bool = False
                if self.on_key_down_fn is not None and event_type == KEYDOWN:
                    self.state = self.on_key_down_fn(self.state, key)
                    handled = True
                if self.on_key_press_fn is not None and event_type == KEYPRESS:
                    self.state = self.on_key_press_fn(self.state, key)
                    handled = True
                if self.on_key_up_fn is not None and event_type == KEYUP:
                    self.state = self.on_key_up_fn(self.state, key)
                    handled = True
                if handled:
                    self._show()

            if self.stop_when_fn is not None and self.stop_when_fn(self.state):
                self._show()
                return

            wait_ms: int = min(next_tick_at - now_ms(), KEY_POLL_PERIOD_MS)
            if wait_ms > 0:
                sleep(wait_ms)

    def _show(self) -> None:
        show(self.to_image(self.state))


def animate(create_image: Callable[[int], Image]) -> None:
    period_ms: int = MS_PER_SECOND // DEFAULT_TICK_RATE
    next_frame_at: int = now_ms()
    frame: int = 0
    while True:
        show(create_image(frame))
        next_frame_at += period_ms
        wait_ms: int = next_frame_at - now_ms()
        if wait_ms > 0:
            sleep(wait_ms)
        else:
            next_frame_at = now_ms()
        frame += 1
