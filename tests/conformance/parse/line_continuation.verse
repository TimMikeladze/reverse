# source: written for this project from the Book of Verse, Expressions
# uefn-verified: no
Sum(A : int, B : int, C : int) : int =
    A +
        B +
        C

Call() : int =
    Sum(
        1,
        2,
        3
    )

Items : []int = array{
    1,
    2,
    3
}
