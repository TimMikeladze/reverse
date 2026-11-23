TestClampInside()<decides><transacts> : void = Clamp(5, 0, 10) = 5
TestClampLow()<decides><transacts> : void = Clamp(-3, 0, 10) = 0
TestClampHigh()<decides><transacts> : void = Clamp(99, 0, 10) = 10
TestClampEdges()<decides><transacts> : void =
    Clamp(0, 0, 10) = 0
    Clamp(10, 0, 10) = 10
