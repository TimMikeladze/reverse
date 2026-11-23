# source: written for this project from the Book of Verse, Classes & Interfaces
# uefn-verified: no
#> hi ann
#> cast to cat failed
#> cast to dog worked: woof
named := interface:
    Label() : string
    Greet() : string = "hi " + Label()

person := class(named):
    Label<override>() : string = "ann"

animal := class<abstract>:
    Sound() : string

dog := class(animal):
    Sound<override>() : string = "woof"

cat := class(animal):
    Sound<override>() : string = "meow"

Main() : void =
    P := person{}
    Print(P.Greet())
    A : animal = dog{}
    if (C := cat[A]):
        Print("unexpected {C.Sound()}")
    else:
        Print("cast to cat failed")
    if (D := dog[A]):
        Print("cast to dog worked: {D.Sound()}")
