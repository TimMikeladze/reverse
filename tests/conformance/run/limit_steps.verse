# source: written for this project from the Book of Verse, (resource limits are a property of this interpreter)
# uefn-verified: no
#> start
#= status: error:step_limit
Main() : void =
    Print("start")
    var I : int = 0
    loop:
        set I += 1
        if (I < 0) then break
