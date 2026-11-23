# source: Book of Verse, Failure and Concurrency
# uefn-verified: no

TryOpen(Allowed : logic)<decides><transacts> : void =
    SetDoorOpen[true]
    DoorOpen[]?
    DoorOpened()
    Allowed?

OnInteract(Allowed : logic)<suspends> : void =
    if (TryOpen[Allowed]):
        Sleep(0.5)
        if (SetDoorOpen[false]) {}
