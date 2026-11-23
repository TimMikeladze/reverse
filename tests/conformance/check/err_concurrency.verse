# source: written for this project from the Book of Verse, Concurrency and Effects
# uefn-verified: no
Work()<suspends> : int =
    Sleep(1.0)
    1

A() : int = Work() #! V4001
B()<suspends> : int =
    if (X := Work(), X > 0) then X else 0 #! V4006
C()<suspends> : ?int = option{Work()} #! V4006
D()<suspends> : void =
    sync: #! V2002
        Work()
E()<suspends> : void =
    loop:
        branch: #! V4007
            Work()
        Sleep(1.0)
F()<suspends> : void =
    Ev := event(int){}
    Ev.Signal("text") #! V2001
    Ev.Cancel() #! V1003
G() : void =
    Ev := event(int){}
    X := Ev.Await() #! V4001
H()<suspends> : string =
    T := spawn{Work()}
    T.Await() #! V2006
