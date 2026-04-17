from dataclasses import dataclass


@dataclass
class Performance:
    team: str
    points: int
    wins: int
    goal_diff: int


def best_performance(a: Performance, b: Performance) -> str:
    '''
    Returns the name of the team with the best performance between *a* and *b*.
    Tiebreaker: most points, then wins, then goal difference, then alphabetical.

    Examples
    >>> best_performance(Performance('City', 4, 1, 2), Performance('United', 1, 0, -2))
    'City'
    >>> best_performance(Performance('City', 4, 1, 2), Performance('United', 4, 2, 2))
    'United'
    >>> best_performance(Performance('City', 5, 2, 2), Performance('United', 5, 2, 1))
    'City'
    >>> best_performance(Performance('City', 5, 2, 2), Performance('United', 5, 2, 2))
    'City'
    '''
    if a.points > b.points or \
            a.points == b.points and a.wins > b.wins or \
            a.points == b.points and a.wins == b.wins and a.goal_diff > b.goal_diff or \
            a.points == b.points and a.wins == b.wins and a.goal_diff == b.goal_diff and a.team < b.team:
        team = a.team
    else:
        team = b.team
    return team


def update_performance(d: Performance, goals_scored: int, goals_conceded: int) -> Performance:
    '''
    Returns a new performance updated from *d* with the given match result.
    Win (scored > conceded): +3 points, +1 win. Draw: +1 point.

    Examples
    >>> update_performance(Performance('Rovers', 5, 1, -1), 4, 2)
    Performance(team='Rovers', points=8, wins=2, goal_diff=1)
    >>> update_performance(Performance('Rovers', 8, 2, 1), 3, 3)
    Performance(team='Rovers', points=9, wins=2, goal_diff=1)
    >>> update_performance(Performance('Rovers', 8, 2, 1), 1, 4)
    Performance(team='Rovers', points=8, wins=2, goal_diff=-2)
    '''
    points = d.points
    wins = d.wins
    goal_diff = d.goal_diff + goals_scored - goals_conceded
    if goals_scored > goals_conceded:
        points = points + 3
        wins = wins + 1
    elif goals_scored == goals_conceded:
        points = points + 1
    return Performance(d.team, points, wins, goal_diff)


assert best_performance(Performance('City', 4, 1, 2), Performance('United', 1, 0, -2)) == 'City'
assert best_performance(Performance('City', 4, 1, 2), Performance('United', 4, 2, 2)) == 'United'
assert best_performance(Performance('City', 5, 2, 2), Performance('United', 5, 2, 1)) == 'City'
assert best_performance(Performance('City', 5, 2, 2), Performance('United', 5, 2, 2)) == 'City'
assert update_performance(Performance('Rovers', 5, 1, -1), 4, 2) == Performance('Rovers', 8, 2, 1)
assert update_performance(Performance('Rovers', 8, 2, 1), 3, 3) == Performance('Rovers', 9, 2, 1)
assert update_performance(Performance('Rovers', 8, 2, 1), 1, 4) == Performance('Rovers', 8, 2, -2)
