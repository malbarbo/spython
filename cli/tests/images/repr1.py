from spython import (
    BOLD,
    ITALIC,
    Color,
    FillRule,
    Font,
    LineCap,
    LineJoin,
    blue,
    circle,
    fill,
    green,
    join,
    none,
    red,
    stroke,
)

print(red)
print(Color(1, 2, 3, 0.5))
print(Font())
print(Font(family="mono", size=20, style=ITALIC, weight=BOLD, underline=True))
print(none)
print(fill(red, opacity=0.5, rule=FillRule.EVENODD))
print(
    stroke(
        blue,
        width=2,
        opacity=0.25,
        dash_array=[1, 2.5],
        linecap=LineCap.ROUND,
        linejoin=LineJoin.BEVEL,
    )
)
s = join(fill(red), stroke(blue), none, fill(green, opacity=0.1))
print(s)
print(eval(repr(s)) == s)
print([circle(5, fill(red))])
