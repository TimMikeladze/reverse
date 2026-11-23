PairsWithSum(Values : []int, Target : int) : []tuple(int, int) =
    for (I -> A : Values, J -> B : Values, I < J, A + B = Target) { (A, B) }
