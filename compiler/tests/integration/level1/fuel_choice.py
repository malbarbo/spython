def fuel_choice(ethanol_price: float, gasoline_price: float) -> str:
    '''
    Returns the recommended fuel. Produces 'ethanol' if *ethanol_price*
    is at most 70% of *gasoline_price*, otherwise produces 'gasoline'.

    Examples
    >>> fuel_choice(4.00, 6.00)  # 4.00 <= 0.7 * 6.00
    'ethanol'
    >>> fuel_choice(3.50, 5.00)  # 3.50 <= 0.7 * 5.00
    'ethanol'
    >>> fuel_choice(4.00, 5.00)  # 4.00 > 0.7 * 5.00
    'gasoline'
    '''
    if ethanol_price <= 0.7 * gasoline_price:
        fuel = 'ethanol'
    else:
        fuel = 'gasoline'
    return fuel


assert fuel_choice(4.00, 6.00) == 'ethanol'
assert fuel_choice(3.50, 5.00) == 'ethanol'
assert fuel_choice(4.00, 5.00) == 'gasoline'
