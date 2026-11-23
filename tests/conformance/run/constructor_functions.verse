# source: written for this project from the Book of Verse, Classes & Interfaces (constructor functions)
# uefn-verified: no
#> ann at level 5
#> guest at level 1
player := class:
    Name : string
    Level : int

MakePlayer<constructor>(N : string, L : int) := player:
    Name := N
    Level := L

MakeGuest<constructor>() := player:
    Name := "guest"
    Level := 1

Main() : void =
    P := MakePlayer("ann", 5)
    Print("{P.Name} at level {P.Level}")
    G := MakeGuest()
    Print("{G.Name} at level {G.Level}")
