CharCounts(Text : string) : [char]int =
    var Counts : [char]int = map{}
    for (C : Text):
        Old := Counts[C] or 0
        if (set Counts[C] = Old + 1) {}
    Counts
