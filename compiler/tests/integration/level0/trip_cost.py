def trip_cost(distance: float, efficiency: float, price: float) -> float:
    '''
    Calculates the cost to travel *distance* (km) considering the vehicle
    *efficiency* (km/L) and the fuel *price* per liter.

    Examples
    >>> # (120.0 / 10.0) * 5.0
    >>> trip_cost(120.0, 10.0, 5.0)
    60.0
    >>> # (300.0 / 15.0) * 6.0
    >>> trip_cost(300.0, 15.0, 6.0)
    120.0
    '''
    return (distance / efficiency) * price


assert trip_cost(120.0, 10.0, 5.0) == 60.0
assert trip_cost(300.0, 15.0, 6.0) == 120.0
