# source: written for this project from the Book of Verse, (deadlock detection is a property of this scheduler)
# uefn-verified: no
#> waiting for a signal that never comes
#= status: error:deadlock
Main()<suspends> : void =
    E : event(int) = event(int){}
    Print("waiting for a signal that never comes")
    V := E.Await()
    Print("unreachable {V}")
