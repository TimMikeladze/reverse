# source: Epic's public Verse API reference, event(t).Subscribe and cancelable.Cancel; calling handlers synchronously in subscription order is this toolchain's choice
# uefn-verified: no
#> pressed by zed #1
#> pressed by zed #2
#> awaited zed
#> later zed
#> later zed
#> later zed
#> count 2
agent := class<unique>:
    Name : string = "a"

button := class:
    InteractedWithEvent : event(agent) = event(agent){}
    Done : listenable(void) = event(){}
    SimInteract(A : agent) : void =
        InteractedWithEvent.Signal(A)

dev := class:
    B : button = button{}
    var Sub : ?cancelable = false
    var Count : int = 0
    Begin()<suspends> : void =
        set Sub = option{B.InteractedWithEvent.Subscribe(OnPress)}
        B.InteractedWithEvent.Subscribe(OnPressSpawn)
        A := B.InteractedWithEvent.Await()
        Print("awaited {A.Name}")
    OnPress(A : agent) : void =
        set Count += 1
        Print("pressed by {A.Name} #{Count}")
        if (Count >= 2, S := Sub?):
            S.Cancel()
    OnPressSpawn(A : agent) : void =
        spawn{Later(A)}
    Later(A : agent)<suspends> : void =
        Sleep(1.0)
        Print("later {A.Name}")

Main()<suspends> : void =
    D := dev{}
    spawn{D.Begin()}
    Sleep(0.0)
    Z := agent{Name := "zed"}
    D.B.SimInteract(Z)
    D.B.SimInteract(Z)
    D.B.SimInteract(Z)
    Sleep(2.0)
    Print("count {D.Count}")
