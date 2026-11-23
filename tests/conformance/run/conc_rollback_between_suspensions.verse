# source: written for this project from the Book of Verse, Concurrency and Failure
# uefn-verified: no
#> after failed attempt 0
#> after successful attempt 5
box := class:
    var V : int = 0

TrySet(B : box, X : int)<decides><transacts> : void =
    set B.V = X
    X > 3

Main()<suspends> : void =
    B := box{}
    Sleep(1.0)
    # Failure contexts work as usual inside suspending code.
    if (TrySet[B, 2]) {}
    Print("after failed attempt {B.V}")
    Sleep(1.0)
    if (TrySet[B, 5]) {}
    Print("after successful attempt {B.V}")
