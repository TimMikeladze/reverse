# source: written for this project from the Book of Verse, Failure and Primitive types
# uefn-verified: no

Charge(Allowed : logic)<decides><transacts> : void =
    Allowed?
    Previous := GetUnits[]
    Previous < 3
    Next := Previous + 1
    SetUnits[Next]
    SetActive[true]
    SetLabel["CHARGED"]
    Report("Relay charge {Next}/3")

OnInteract(Allowed : logic) : void =
    if (Charge[Allowed]) {}
