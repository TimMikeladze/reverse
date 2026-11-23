stack := class:
    var Items : []int = array{}

    Push(Value : int) : void =
        set Items += array{Value}

    Pop()<decides><transacts> : int =
        Last := Items.Length - 1
        Value := Items[Last]
        set Items = Items.RemoveElement[Last]
        Value

    Size() : int = Items.Length
