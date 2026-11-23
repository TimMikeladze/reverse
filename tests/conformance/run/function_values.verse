# source: written for this project from the Book of Verse, Functions
# uefn-verified: no
#> 8
#> 9
Double(X : int) : int = X * 2
Square(X : int) : int = X * X

Apply(F : type{_(:int):int}, X : int) : int = F(X)

Main() : void =
    Print("{Apply(Double, 4)}")
    Print("{Apply(Square, 3)}")
