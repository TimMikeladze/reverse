Clamp(Value : int, Low : int, High : int) : int =
    if (Value < Low) then Low else if (Value > High) then High else Value
