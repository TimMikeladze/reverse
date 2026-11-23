# source: written for this project from the Book of Verse, Classes & Interfaces
# uefn-verified: no
#> rex says woof
#> tiny says yip, then woof
#> shared reference: 2 2
#> block ran: id-7
#> legs 4
animal := class<abstract>:
    Name : string
    Legs : int = 4
    Sound() : string
    Describe() : string = "{Name} says {Sound()}"

dog := class(animal):
    Sound<override>() : string = "woof"

puppy := class(dog):
    Sound<override>() : string = "yip, then {(super:)Sound()}"

counter := class:
    var N : int = 0
    Bump()<transacts> : void = set N += 1

tagged := class:
    ID : int
    var Label : string = ""
    block:
        set Label = "id-{ID}"

Main() : void =
    Rex : animal = dog{Name := "rex"}
    Tiny : animal = puppy{Name := "tiny"}
    # Describe is inherited; Sound dispatches on the runtime class.
    Print(Rex.Describe())
    Print(Tiny.Describe())
    A := counter{}
    B := A
    A.Bump()
    B.Bump()
    Print("shared reference: {A.N} {B.N}")
    Print("block ran: {tagged{ID := 7}.Label}")
    Print("legs {Rex.Legs}")
