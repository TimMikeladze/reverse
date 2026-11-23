# source: written for this project from the Book of Verse, Control Flow and Structs & Enums
# uefn-verified: no
#> north south other
#> one many
#> yes
direction := enum{North, South, East, West}

Name(D : direction) : string =
    case (D):
        direction.North => "north"
        direction.South => "south"
        _ => "other"

Count(N : int) : string =
    case (N):
        1 => "one"
        _ => "many"

Main() : void =
    Print("{Name(direction.North)} {Name(direction.South)} {Name(direction.West)}")
    Print("{Count(1)} {Count(7)}")
    if (direction.East = direction.East, direction.East <> direction.West):
        Print("yes")
