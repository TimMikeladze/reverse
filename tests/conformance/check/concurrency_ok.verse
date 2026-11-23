# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
Work()<suspends> : int =
    Sleep(1.0)
    1

Types()<suspends> : void =
    Both : tuple(int, int) = sync:
        Work()
        Work()
    Winner : int = race:
        Work()
        Work()
    T : task(int) = spawn{Work()}
    V : int = T.Await()
    E : event(string) = event(string){}
    E.Signal("go")
    S : string = E.Await()

FromPlainCode() : void =
    spawn{Work()}
    E := event(int){}
    E.Signal(1)
