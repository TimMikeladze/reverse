TestAreas()<decides><transacts> : void =
    rectangle{Width := 2.0, Height := 3.0}.Area() = 6.0
    square{Side := 4.0}.Area() = 16.0
TestTotalArea()<decides><transacts> : void =
    Shapes : []shape = array{rectangle{Width := 2.0, Height := 3.0}, square{Side := 2.0}}
    TotalArea(Shapes) = 10.0
    Empty : []shape = array{}
    TotalArea(Empty) = 0.0
