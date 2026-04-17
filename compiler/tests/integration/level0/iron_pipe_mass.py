PI: float = 3.14
IRON_DENSITY: float = 7874


def iron_pipe_mass(outer_diameter: float, inner_diameter: float, height: float) -> float:
    '''
    Calculates the mass of an iron pipe from its dimensions.

    Requires outer_diameter > inner_diameter.

    Examples
    >>> # 3.14 * ((0.05 / 2) ** 2 - (0.03 / 2) ** 2) * 0.1 * 7874
    >>> round(iron_pipe_mass(0.05, 0.03, 0.1), 7)
    0.9889744
    '''
    outer_area = PI * (outer_diameter / 2) ** 2
    inner_area = PI * (inner_diameter / 2) ** 2
    volume = (outer_area - inner_area) * height
    return volume * IRON_DENSITY


assert round(iron_pipe_mass(0.05, 0.03, 0.1), 7) == 0.9889744
