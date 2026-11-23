# source: written for this project from the Book of Verse, Access Specifiers
# uefn-verified: no
vault := class:
    Code<private> : int = 42
    Hint<protected> : string = "four"
    Label<public> : string = "vault"
    Size : int = 1
    Reveal<public>() : int = Code
    Rotate<private>() : void = {}

strongbox := class(vault):
    Clue() : string = Hint
    Steal() : int = Code #! V7005

A(V : vault) : int = V.Code #! V7005
B(V : vault) : string = V.Hint #! V7005
C(V : vault) : string = V.Label
D(V : vault) : int = V.Size
E(V : vault) : int = V.Reveal()
F(V : vault) : void = V.Rotate() #! V7005
