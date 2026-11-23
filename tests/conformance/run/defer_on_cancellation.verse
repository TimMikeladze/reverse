# source: written for this project from the Book of Verse, Control Flow (defer) and Concurrency
# uefn-verified: no
#> worker started
#> timer won
#> worker cleaned up after being cancelled
#> resource released: true
resource := class:
    var Released : logic = false

Worker(R : resource)<suspends> : void =
    defer:
        set R.Released = true
        Print("worker cleaned up after being cancelled")
    Print("worker started")
    Sleep(100.0)
    Print("never reached")

Main()<suspends> : void =
    R := resource{}
    race:
        Worker(R)
        block:
            Sleep(1.0)
            Print("timer won")
    Sleep(1.0)
    Print("resource released: {if (R.Released?) then "true" else "false"}")
