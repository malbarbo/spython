from enum import Enum, auto


class Move(Enum):
    ROCK = auto()
    PAPER = auto()
    SCISSORS = auto()


class Outcome(Enum):
    DRAW = auto()
    FIRST = auto()
    SECOND = auto()


def rps_outcome(a: Move, b: Move) -> Outcome:
    '''
    Determines the outcome of a rock-paper-scissors round.
    ROCK beats SCISSORS, SCISSORS beats PAPER, PAPER beats ROCK.

    Examples
    >>> rps_outcome(Move.ROCK, Move.ROCK).name
    'DRAW'
    >>> rps_outcome(Move.PAPER, Move.PAPER).name
    'DRAW'
    >>> rps_outcome(Move.SCISSORS, Move.SCISSORS).name
    'DRAW'
    >>> rps_outcome(Move.ROCK, Move.SCISSORS).name
    'FIRST'
    >>> rps_outcome(Move.PAPER, Move.ROCK).name
    'FIRST'
    >>> rps_outcome(Move.SCISSORS, Move.PAPER).name
    'FIRST'
    >>> rps_outcome(Move.ROCK, Move.PAPER).name
    'SECOND'
    >>> rps_outcome(Move.PAPER, Move.SCISSORS).name
    'SECOND'
    >>> rps_outcome(Move.SCISSORS, Move.ROCK).name
    'SECOND'
    '''
    if a == b:
        r = Outcome.DRAW
    elif (a == Move.ROCK and b == Move.SCISSORS) or \
            (a == Move.SCISSORS and b == Move.PAPER) or \
            (a == Move.PAPER and b == Move.ROCK):
        r = Outcome.FIRST
    else:
        r = Outcome.SECOND
    return r


assert rps_outcome(Move.ROCK, Move.ROCK).name == 'DRAW'
assert rps_outcome(Move.PAPER, Move.PAPER).name == 'DRAW'
assert rps_outcome(Move.SCISSORS, Move.SCISSORS).name == 'DRAW'
assert rps_outcome(Move.ROCK, Move.SCISSORS).name == 'FIRST'
assert rps_outcome(Move.PAPER, Move.ROCK).name == 'FIRST'
assert rps_outcome(Move.SCISSORS, Move.PAPER).name == 'FIRST'
assert rps_outcome(Move.ROCK, Move.PAPER).name == 'SECOND'
assert rps_outcome(Move.PAPER, Move.SCISSORS).name == 'SECOND'
assert rps_outcome(Move.SCISSORS, Move.ROCK).name == 'SECOND'
