# Tests for door.verse. They use `Sim...` helpers, which exist only in this
# toolchain, so this file stays out of UEFN.
#
#     reverse test examples/door/door.verse examples/door/door_test.verse

SimNewDoor() : door_device =
    door_device{Clock := timer_device{SimDuration := 30.0}}

TestThreePressesOpenTheDoor()<decides><transacts> : void =
    Ana := SimAddPlayer("Ana")
    Door := SimNewDoor()
    SimStart(Door)
    Door.Button.SimInteract(Ana)
    Door.Button.SimInteract(Ana)
    not Door.Open?
    not SimGameEnded[]
    Door.Button.SimInteract(Ana)
    Door.Open?
    SimWinner[] = Ana
    SimScore(Ana) = 3
    Door.Hud.SimShown[2] = "3 of 3"

TestPressesAreCountedPerPlayer()<decides><transacts> : void =
    Ana := SimAddPlayer("Ana")
    Ben := SimAddPlayer("Ben")
    Door := SimNewDoor()
    SimStart(Door)
    Door.Button.SimInteract(Ana)
    Door.Button.SimInteract(Ben)
    Door.Button.SimInteract(Ana)
    Door.Button.SimInteract(Ben)
    not Door.Open?

# The clock is simulated, so thirty seconds pass at once.
TestTheClockLocksTheDoor()<suspends> : logic =
    Ana := SimAddPlayer("Ana")
    Door := SimNewDoor()
    SimStart(Door)
    Sleep(10.0)
    Door.Button.SimInteract(Ana)
    Sleep(25.0)
    Door.Button.SimInteract(Ana)
    Door.Button.SimInteract(Ana)
    logic{SimScore(Ana) = 1 and not Door.Open? and Door.Hud.SimShown.Length = 2}
