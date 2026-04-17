from dataclasses import dataclass
from enum import Enum, auto


@dataclass
class Resolution:
    height: int
    width: int


class Aspect(Enum):
    A3x4 = auto()
    A16x9 = auto()
    OTHER = auto()


def megapixels(r: Resolution) -> float:
    '''
    Returns the number of megapixels of an image with resolution *r*.

    Examples
    >>> megapixels(Resolution(360, 640))
    0.2304
    >>> megapixels(Resolution(1024, 768))
    0.786432
    '''
    return r.height * r.width / 1000000


def image_fits_screen(img: Resolution, screen: Resolution) -> bool:
    '''
    Returns True if the image fits on the screen without rotation or scaling.

    Examples
    >>> image_fits_screen(Resolution(300, 400), Resolution(330, 450))
    True
    >>> image_fits_screen(Resolution(330, 450), Resolution(330, 450))
    True
    >>> image_fits_screen(Resolution(331, 400), Resolution(330, 450))
    False
    >>> image_fits_screen(Resolution(330, 451), Resolution(330, 450))
    False
    '''
    return img.height <= screen.height and img.width <= screen.width


def aspect(r: Resolution) -> Aspect:
    '''
    Returns the aspect ratio of *r*.

    Examples
    >>> aspect(Resolution(1024, 768)).name
    'A3x4'
    >>> aspect(Resolution(1080, 1920)).name
    'A16x9'
    >>> aspect(Resolution(600, 600)).name
    'OTHER'
    '''
    if r.height * 3 == r.width * 4:
        a = Aspect.A3x4
    elif r.height * 16 == r.width * 9:
        a = Aspect.A16x9
    else:
        a = Aspect.OTHER
    return a


assert megapixels(Resolution(360, 640)) == 0.2304
assert megapixels(Resolution(1024, 768)) == 0.786432
assert image_fits_screen(Resolution(300, 400), Resolution(330, 450)) == True
assert image_fits_screen(Resolution(330, 450), Resolution(330, 450)) == True
assert image_fits_screen(Resolution(331, 400), Resolution(330, 450)) == False
assert image_fits_screen(Resolution(330, 451), Resolution(330, 450)) == False
assert aspect(Resolution(1024, 768)).name == 'A3x4'
assert aspect(Resolution(1080, 1920)).name == 'A16x9'
assert aspect(Resolution(600, 600)).name == 'OTHER'
