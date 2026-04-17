from enum import Enum, auto


class Fuel(Enum):
    '''The type of fuel used for refueling.'''
    ETHANOL = auto()
    GASOLINE = auto()


def fuel_choice(ethanol_price: float, gasoline_price: float) -> Fuel:
    '''
    Returns the recommended fuel. Produces Fuel.ETHANOL if *ethanol_price*
    is at most 70% of *gasoline_price*, otherwise produces Fuel.GASOLINE.

    Examples
    >>> fuel_choice(4.00, 6.00).name
    'ETHANOL'
    >>> fuel_choice(3.50, 5.00).name
    'ETHANOL'
    >>> fuel_choice(4.00, 5.00).name
    'GASOLINE'
    '''
    if ethanol_price <= 0.7 * gasoline_price:
        fuel = Fuel.ETHANOL
    else:
        fuel = Fuel.GASOLINE
    return fuel


assert fuel_choice(4.00, 6.00).name == 'ETHANOL'
assert fuel_choice(3.50, 5.00).name == 'ETHANOL'
assert fuel_choice(4.00, 5.00).name == 'GASOLINE'
