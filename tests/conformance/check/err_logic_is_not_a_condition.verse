# source: written for this project from the Book of Verse, Failure and Primitive types
# uefn-verified: no
A(Flag : logic) : int =
    if (Flag) then 1 else 0 #! V3004

B(Flag : logic) : int =
    if (not Flag) then 1 else 0 #! V3004

C(Flag : logic) : int =
    if (Flag?) then 1 else 0

D(Flag : logic) : int =
    if (not Flag?) then 1 else 0

E(Flag : logic) : int =
    if (Flag = true) then 1 else 0
