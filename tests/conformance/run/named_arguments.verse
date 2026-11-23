# source: written for this project from the Book of Verse, Functions
# uefn-verified: no
#> 10
#> 15
#> ann!
Total(Base : int, ?Bonus : int = 0) : int = Base + Bonus
Greet(Name : string, ?Suffix : string = "!") : string = Name + Suffix

Main() : void =
    Print("{Total(10)}")
    Print("{Total(10, ?Bonus := 5)}")
    Print(Greet("ann"))
