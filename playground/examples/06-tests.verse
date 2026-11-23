# Press Test. A <decides> test passes when every line succeeds,
# so comparisons work as assertions.

Clamp(Value : int, Low : int, High : int) : int =
    if (Value < Low) then Low else if (Value > High) then High else Value

(Xs : []int).Sum() : int =
    var Total : int = 0
    for (X : Xs):
        set Total += X
    Total

First(Xs : []t where t : type)<decides> : t = Xs[0]

TestClamp()<decides><transacts> : void =
    Clamp(5, 0, 10) = 5
    Clamp(-3, 0, 10) = 0
    Clamp(99, 0, 10) = 10

TestSum()<decides><transacts> : void =
    array{1, 2, 3}.Sum() = 6
    Empty : []int = array{}
    Empty.Sum() = 0

TestFirst()<decides><transacts> : void =
    First[array{"a", "b"}] = "a"
    Empty : []int = array{}
    not First[Empty]

# This one fails on purpose: change 4 to 5 and test again.
TestBroken()<decides><transacts> : void =
    Clamp(5, 0, 4) = 5
