from enum import Flag
class P(Flag):
    R = 1
    W = 2
print(P.R | P.W)
