# source: written for this project from the Book of Verse, Classes & Interfaces
# uefn-verified: no
named := interface:
    Label() : string
    Greet() : string = "hi " + Label()

animal := class<abstract>(named):
    Legs : int = 4
    Sound() : string

dog := class(animal):
    Label<override>() : string = "dog"
    Sound<override>() : string = "woof"

puppy := class(dog):
    Sound<override>() : string = "yip, then " + (super:)Sound()

Use(A : animal) : string =
    if (D := dog[A]):
        D.Sound()
    else:
        A.Greet()

Make() : animal = puppy{}
