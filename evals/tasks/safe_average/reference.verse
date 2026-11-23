Average(Values : []int)<decides> : float =
    Values.Length > 0
    var Sum : int = 0
    for (V : Values):
        set Sum += V
    (Sum * 1.0) / (Values.Length * 1.0)
