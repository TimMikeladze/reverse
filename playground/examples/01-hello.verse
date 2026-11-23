# Edit the code: it is checked as you type.
# Run executes Main(); Test runs every Test...() function.

Greeting : string = "Hello"

Describe(Score : int) : string =
    if (Score >= 10):
        "{Greeting}, champion: {Score} points"
    else:
        "{Greeting}: {Score} points"

Main() : void =
    var Total : int = 0
    for (Points : array{3, 4, 5}):
        set Total += Points
        Print(Describe(Total))
    Squares := for (I := 1..5) { I * I }
    Print("{Squares.Length} squares, the last is {Squares[4] or 0}")
