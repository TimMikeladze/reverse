TestAverage()<decides><transacts> : void =
    Average[array{1, 2, 3, 4}] = 2.5
    Average[array{10}] = 10.0
    Average[array{-2, 2}] = 0.0
TestAverageEmptyFails()<decides><transacts> : void =
    Empty : []int = array{}
    not Average[Empty]
