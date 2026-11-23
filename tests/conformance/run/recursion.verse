# source: written for this project from the Book of Verse, Functions
# uefn-verified: no
#> fact 3628800
#> fib 55
#> even
Fact(N : int) : int = if (N <= 1) then 1 else N * Fact(N - 1)
Fib(N : int) : int = if (N < 2) then N else Fib(N - 1) + Fib(N - 2)
IsEven(N : int) : logic = if (N = 0) then true else IsOdd(N - 1)
IsOdd(N : int) : logic = if (N = 0) then false else IsEven(N - 1)

Main() : void =
    Print("fact {Fact(10)}")
    Print("fib {Fib(10)}")
    if (IsEven(10)?):
        Print("even")
