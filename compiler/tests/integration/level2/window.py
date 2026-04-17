from dataclasses import dataclass


@dataclass
class Window:
    x: int
    y: int
    width: int
    height: int


@dataclass
class Click:
    x: int
    y: int


def inside_window(w: Window, c: Click) -> bool:
    '''
    Returns True if the click *c* is inside the window *w*.

    Examples
    >>> win = Window(100, 100, 300, 200)
    >>> inside_window(win, Click(150, 150))
    True
    >>> inside_window(win, Click(600, 150))
    False
    >>> inside_window(win, Click(150, 300))
    False
    >>> inside_window(win, Click(100, 100))
    True
    >>> inside_window(win, Click(399, 100))
    True
    >>> inside_window(win, Click(400, 100))
    False
    >>> inside_window(win, Click(399, 299))
    True
    >>> inside_window(win, Click(399, 300))
    False
    '''
    return w.x <= c.x < (w.x + w.width) and w.y <= c.y < (w.y + w.height)


def windows_overlap(a: Window, b: Window) -> bool:
    '''
    Returns True if windows *a* and *b* overlap.

    Examples
    >>> windows_overlap(Window(10, 250, 100, 200), Window(300, 400, 50, 100))
    False
    >>> windows_overlap(Window(210, 250, 100, 200), Window(300, 400, 50, 100))
    True
    >>> windows_overlap(Window(310, 250, 100, 200), Window(300, 400, 50, 100))
    True
    >>> windows_overlap(Window(410, 250, 100, 200), Window(300, 400, 50, 100))
    False
    '''
    return a.x < (b.x + b.width) and \
           b.x < (a.x + a.width) and \
           a.y < (b.y + b.height) and \
           b.y < (a.y + a.height)


_win = Window(100, 100, 300, 200)
assert inside_window(_win, Click(150, 150)) == True
assert inside_window(_win, Click(600, 150)) == False
assert inside_window(_win, Click(150, 300)) == False
assert inside_window(_win, Click(100, 100)) == True
assert inside_window(_win, Click(399, 100)) == True
assert inside_window(_win, Click(400, 100)) == False
assert inside_window(_win, Click(399, 299)) == True
assert inside_window(_win, Click(399, 300)) == False
assert windows_overlap(Window(10, 250, 100, 200), Window(300, 400, 50, 100)) == False
assert windows_overlap(Window(210, 250, 100, 200), Window(300, 400, 50, 100)) == True
assert windows_overlap(Window(310, 250, 100, 200), Window(300, 400, 50, 100)) == True
assert windows_overlap(Window(410, 250, 100, 200), Window(300, 400, 50, 100)) == False
