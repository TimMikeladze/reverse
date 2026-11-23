TestFirstEvenFound()<decides><transacts> : void =
    FirstEven(array{3, 5, 8, 10})? = 8
    FirstEven(array{2})? = 2
    FirstEven(array{-4, 1})? = -4
TestFirstEvenMissing()<decides><transacts> : void =
    not FirstEven(array{1, 3, 5})?
    Empty : []int = array{}
    not FirstEven(Empty)?
