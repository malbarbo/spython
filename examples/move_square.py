from dataclasses import dataclass

from spython import (
    Image,
    World,
    black,
    create_world,
    empty_scene,
    fill,
    join,
    place_image_align,
    red,
    square,
    stroke,
    LEFT,
    TOP,
)

LINES: int = 9
COLUMNS: int = 11
SIZE: int = 30


@dataclass
class Pos:
    line: int
    column: int


def draw(p: Pos) -> Image:
    return place_image_align(
        empty_scene(SIZE * COLUMNS, SIZE * LINES),
        SIZE * p.column,
        SIZE * p.line,
        LEFT,
        TOP,
        square(SIZE, join(fill(red), stroke(black))),
    )


def move(p: Pos, key: str) -> Pos:
    if key == "ArrowLeft":
        p = Pos(p.line, p.column - 1)
    elif key == "ArrowRight":
        p = Pos(p.line, p.column + 1)
    elif key == "ArrowDown":
        p = Pos(p.line + 1, p.column)
    elif key == "ArrowUp":
        p = Pos(p.line - 1, p.column)
    return Pos(
        max(0, min(LINES - 1, p.line)),
        max(0, min(COLUMNS - 1, p.column)),
    )


w: World = create_world(Pos(LINES // 2, COLUMNS // 2), draw)
w.on_key_down(move)
w.stop_when(lambda p: p.line == 0 and p.column == 0)
w.run()
