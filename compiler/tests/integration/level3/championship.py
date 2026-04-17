from dataclasses import dataclass


@dataclass
class Performance:
    team: str
    points: int
    wins: int
    goal_diff: int


@dataclass
class MatchResult:
    goals_scored: int
    goals_conceded: int


def update_performance(d: Performance, goals_scored: int, goals_conceded: int) -> Performance:
    '''
    Returns an updated performance from *d* with a new match result.
    Win: +3 points, +1 win. Draw: +1 point. Goal diff always updated.

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


def calc_performance(team: str, results: list[MatchResult]) -> Performance:
    '''
    Calculates the performance of *team* from its match *results*.

    Examples
    >>> calc_performance('Rovers', [])
    Performance(team='Rovers', points=0, wins=0, goal_diff=0)
    >>> calc_performance('City', [MatchResult(4, 1), MatchResult(3, 3), MatchResult(0, 1)])
    Performance(team='City', points=4, wins=1, goal_diff=2)
    '''
    d = Performance(team, 0, 0, 0)
    for r in results:
        d = update_performance(d, r.goals_scored, r.goals_conceded)
    return d


assert update_performance(Performance('Rovers', 5, 1, -1), 4, 2) == Performance('Rovers', 8, 2, 1)
assert update_performance(Performance('Rovers', 8, 2, 1), 3, 3) == Performance('Rovers', 9, 2, 1)
assert update_performance(Performance('Rovers', 8, 2, 1), 1, 4) == Performance('Rovers', 8, 2, -2)
assert calc_performance('Rovers', []) == Performance('Rovers', 0, 0, 0)
assert calc_performance('City', [MatchResult(4, 1), MatchResult(3, 3), MatchResult(0, 1)]) == Performance('City', 4, 1, 2)
