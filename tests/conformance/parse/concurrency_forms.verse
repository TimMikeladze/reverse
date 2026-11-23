# source: written for this project from the Book of Verse, Concurrency
# uefn-verified: no
Work()<suspends> : int =
    Sleep(1.0)
    1

All()<suspends> : void =
    Both := sync:
        Work()
        Work()
    First := race:
        Work()
        Work()
    rush:
        Work()
        Work()
    branch:
        Work()

Start() : void =
    spawn{Work()}
