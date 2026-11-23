# source: written for this project from the Book of Verse, Concurrency (a round timer racing a win condition)
# uefn-verified: no
#> point 1 at 3.000000
#> point 2 at 6.000000
#> point 3 at 9.000000
#> round over: scored 3 before the 10 second timer
#> final time 9.000000
game := class:
    var Score : int = 0
    Won : event(int) = event(int){}

    Play()<suspends> : void =
        loop:
            Sleep(3.0)
            set Score += 1
            Print("point {Score} at {GetSimulationElapsedTime()}")
            if (Score >= 3):
                Won.Signal(Score)
                break

    Round()<suspends> : string =
        branch:
            Play()
        race:
            block:
                Final := Won.Await()
                "scored {Final} before the 10 second timer"
            block:
                Sleep(10.0)
                "timer ran out"

Main()<suspends> : void =
    G := game{}
    Print("round over: {G.Round()}")
    Print("final time {GetSimulationElapsedTime()}")
