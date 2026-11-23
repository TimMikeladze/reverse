# source: written for this project from the Book of Verse, Control Flow
# uefn-verified: no
suit := enum{Hearts, Spades}

A(N : int) : string =
    case (N):
        0 => "zero"
        1 => "one"
        _ => "many"

B(S : suit) : int =
    case (S) {
        suit.Hearts => 1
        suit.Spades => 2
    }
