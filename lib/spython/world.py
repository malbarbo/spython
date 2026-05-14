import time
from collections.abc import Callable

from spython.image import Image
from spython.system import (
    enter_animation,
    exit_animation,
    show,
    wait_event,
)

# Event kinds returned by `wait_event` (matches the encoding in
# `engine/src/host/native.rs::input_event_to_tuple`).
KEYPRESS: int = 0
KEYDOWN: int = 1
KEYUP: int = 2
VSYNC: int = 3
CLOSE: int = 4

DEFAULT_TICK_RATE: int = 30


class World[S]:
    def __init__(self, state: S, to_image: Callable[[S], Image]) -> None:
        self.state: S = state
        self.to_image: Callable[[S], Image] = to_image
        self.rate: int = DEFAULT_TICK_RATE
        self.on_tick_fn: Callable[[S], S] | None = None
        self.on_tick_time_fn: Callable[[S, float], S] | None = None
        self.stop_when_fn: Callable[[S], bool] | None = None
        self.on_key_press_fn: Callable[[S, str], S] | None = None
        self.on_key_down_fn: Callable[[S, str], S] | None = None
        self.on_key_up_fn: Callable[[S, str], S] | None = None

    def on_tick(self, handler: Callable[[S], S]) -> "World[S]":
        self.on_tick_fn = handler
        return self

    def on_tick_time(self, handler: Callable[[S, float], S]) -> "World[S]":
        self.on_tick_time_fn = handler
        return self

    def tick_rate(self, rate: int) -> "World[S]":
        self.rate = max(1, rate)
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
        if self.on_tick_fn is not None and self.on_tick_time_fn is not None:
            raise ValueError(
                "define on_tick OR on_tick_time, not both"
            )
        enter_animation()
        try:
            self._run_loop()
        finally:
            exit_animation()

    def _run_loop(self) -> None:
        self._show()  # initial frame

        step: float = 1.0 / self.rate
        accumulator: float = 0.0
        last_time: float = time.time()

        while True:
            ev = wait_event()
            if ev is None:
                return
            kind, key, _alt, _ctrl, _shift, _meta, _repeat = ev
            if kind == CLOSE:
                return

            now: float = time.time()
            elapsed: float = now - last_time
            last_time = now

            new_state: S = self.state

            if kind == VSYNC:
                if self.on_tick_time_fn is not None:
                    new_state = self.on_tick_time_fn(self.state, elapsed)
                elif self.on_tick_fn is not None:
                    accumulator += elapsed
                    if accumulator >= step:
                        new_state = self.on_tick_fn(self.state)
                        accumulator -= step
                        # No catch-up: if we fell more than a full step
                        # behind (e.g. on_tick was slow), drop the excess
                        # instead of running ticks back-to-back.
                        if accumulator > step:
                            accumulator = 0.0
            elif kind == KEYPRESS and self.on_key_press_fn is not None:
                new_state = self.on_key_press_fn(self.state, key)
            elif kind == KEYDOWN and self.on_key_down_fn is not None:
                new_state = self.on_key_down_fn(self.state, key)
            elif kind == KEYUP and self.on_key_up_fn is not None:
                new_state = self.on_key_up_fn(self.state, key)

            if new_state is not self.state:
                self.state = new_state
                self._show()

            if self.stop_when_fn is not None and self.stop_when_fn(self.state):
                return

    def _show(self) -> None:
        show(self.to_image(self.state))


def animate(create_image: Callable[[int], Image]) -> None:
    enter_animation()
    try:
        frame: int = 0
        while True:
            ev = wait_event()
            if ev is None:
                return
            kind = ev[0]
            if kind == VSYNC:
                show(create_image(frame))
                frame += 1
            elif kind == CLOSE:
                return
    finally:
        exit_animation()
