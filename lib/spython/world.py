import time
from collections.abc import Callable
from typing import Any

from spython.image import Image, to_svg
from spython.system import get_key_event, show_svg

KEYPRESS: int = 0
KEYDOWN: int = 1
KEYUP: int = 2

DEFAULT_TICK_RATE: int = 28
MIN_TICK_RATE: int = 1
MAX_TICK_RATE: int = 1000
KEY_EVENT_POLLING_DELAY: int = 10


class World:
    def __init__(self, state: Any, to_image: Callable[[Any], Image]) -> None:
        self.state: Any = state
        self.to_image: Callable[[Any], Image] = to_image
        self.rate: int = DEFAULT_TICK_RATE
        self.on_tick_fn: Callable[[Any], Any] | None = None
        self.stop_when_fn: Callable[[Any], bool] | None = None
        self.on_key_press_fn: Callable[[Any, str], Any] | None = None
        self.on_key_down_fn: Callable[[Any, str], Any] | None = None
        self.on_key_up_fn: Callable[[Any, str], Any] | None = None

    def on_tick(self, handler: Callable[[Any], Any]) -> "World":
        self.on_tick_fn = handler
        return self

    def tick_rate(self, rate: int) -> "World":
        self.rate = max(MIN_TICK_RATE, min(MAX_TICK_RATE, rate))
        return self

    def stop_when(self, handler: Callable[[Any], bool]) -> "World":
        self.stop_when_fn = handler
        return self

    def on_key_press(self, handler: Callable[[Any, str], Any]) -> "World":
        self.on_key_press_fn = handler
        return self

    def on_key_down(self, handler: Callable[[Any, str], Any]) -> "World":
        self.on_key_down_fn = handler
        return self

    def on_key_up(self, handler: Callable[[Any, str], Any]) -> "World":
        self.on_key_up_fn = handler
        return self

    def run(self) -> None:
        self._show()
        time_out: int = MAX_TICK_RATE // self.rate
        while True:
            if time_out <= 0:
                if self.on_tick_fn is not None:
                    self.state = self.on_tick_fn(self.state)
                self._show()
                time_out = MAX_TICK_RATE // self.rate
            else:
                time_out -= KEY_EVENT_POLLING_DELAY

            event: tuple[int, str, bool, bool, bool, bool, bool] | None = (
                get_key_event()
            )
            if event is not None:
                event_type: int = event[0]
                key: str = event[1]
                # event[2..6]: alt, ctrl, shift, meta, repeat
                if self.on_key_down_fn is not None and event_type == KEYDOWN:
                    self.state = self.on_key_down_fn(self.state, key)
                if self.on_key_press_fn is not None and event_type == KEYPRESS:
                    self.state = self.on_key_press_fn(self.state, key)
                if self.on_key_up_fn is not None and event_type == KEYUP:
                    self.state = self.on_key_up_fn(self.state, key)

            if self.stop_when_fn is not None and self.stop_when_fn(self.state):
                self._show()
                return

            time.sleep(KEY_EVENT_POLLING_DELAY / 1000.0)

    def _show(self) -> None:
        show_svg(to_svg(self.to_image(self.state)))


def animate(create_image: Callable[[int], Image]) -> None:
    delay: float = 1.0 / DEFAULT_TICK_RATE
    frame: int = 0
    while True:
        show_svg(to_svg(create_image(frame)))
        time.sleep(delay)
        frame += 1
