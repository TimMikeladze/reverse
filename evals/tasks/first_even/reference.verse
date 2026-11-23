FirstEven(Values : []int) : ?int =
    Evens := for (V : Values, Mod[V, 2] = 0) { V }
    option{Evens[0]}
