# source: written for this project from the Book of Verse, Failure and Concurrency
# uefn-verified: no

TryOpen(Allowed : logic)<decides><transacts> : void =
    not (GetActive[]?)
    SetActive[true]
    GetActive[]?
    SetLabel["OPEN"]
    Report("Gate opened for 3 seconds")
    Allowed?

OnInteract(Allowed : logic)<suspends> : void =
    if (TryOpen[Allowed]):
        Sleep(3.0)
        if (SetActive[false], SetLabel["LOCKED"]):
            Report("Gate timer elapsed")
    else:
        if (not (Allowed?)):
            Print("Gate denied. Staged writes and message rolled back.")
