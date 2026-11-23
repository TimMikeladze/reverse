TestPairs()<decides><transacts> : void =
    PairsWithSum(array{1, 2, 3, 4}, 5) = array{(1, 4), (2, 3)}
    PairsWithSum(array{5, 5, 5}, 10).Length = 3
TestNoPairs()<decides><transacts> : void =
    PairsWithSum(array{1, 2}, 100).Length = 0
    Empty : []int = array{}
    PairsWithSum(Empty, 0).Length = 0
