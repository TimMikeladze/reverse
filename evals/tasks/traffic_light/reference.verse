light := enum{Red, Green, Yellow}

Next(L : light) : light =
    case (L):
        light.Red => light.Green
        light.Green => light.Yellow
        light.Yellow => light.Red

Describe(L : light) : string =
    case (L):
        light.Red => "stop"
        light.Green => "go"
        light.Yellow => "slow"
