from dataclasses import dataclass
from enum import Enum, auto


class Op(Enum):
    ADD = auto()
    MUL = auto()
    NEG = auto()


@dataclass
class Expr:
    op: Op
    args: list[int]


def eval_expr(e: Expr) -> int:
    '''
    Evaluates *e*: ADD sums all args, MUL multiplies all, NEG negates the single arg.

    Examples
    >>> eval_expr(Expr(Op.ADD, [3, 4]))
    7
    >>> eval_expr(Expr(Op.ADD, [1, 2, 3]))
    6
    >>> eval_expr(Expr(Op.MUL, [3, 4]))
    12
    >>> eval_expr(Expr(Op.NEG, [5]))
    -5
    >>> eval_expr(Expr(Op.NEG, [-3]))
    3
    '''
    match e:
        case Expr(op=Op.NEG, args=[x]):
            result = -x
        case Expr(op=Op.ADD, args=values):
            result = 0
            for v in values:
                result = result + v
        case Expr(op=Op.MUL, args=values):
            result = 1
            for v in values:
                result = result * v
    return result


def describe_command(cmd: list[int]) -> str:
    '''
    Interprets a command encoded as a list.
    [0] = stop, [1, x, y] = move to (x,y), [2, angle] = rotate.

    Examples
    >>> describe_command([0])
    'stop'
    >>> describe_command([1, 3, 4])
    'move to (3, 4)'
    >>> describe_command([2, 90])
    'rotate 90'
    '''
    match cmd:
        case [0]:
            desc = 'stop'
        case [1, x, y]:
            desc = 'move to (' + str(x) + ', ' + str(y) + ')'
        case [2, angle]:
            desc = 'rotate ' + str(angle)
        case _:
            desc = 'unknown'
    return desc


assert eval_expr(Expr(Op.ADD, [3, 4])) == 7
assert eval_expr(Expr(Op.ADD, [1, 2, 3])) == 6
assert eval_expr(Expr(Op.MUL, [3, 4])) == 12
assert eval_expr(Expr(Op.NEG, [5])) == -5
assert eval_expr(Expr(Op.NEG, [-3])) == 3
assert describe_command([0]) == 'stop'
assert describe_command([1, 3, 4]) == 'move to (3, 4)'
assert describe_command([2, 90]) == 'rotate 90'
