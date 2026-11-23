shape := class<abstract>:
    Area() : float

rectangle := class(shape):
    Width : float
    Height : float
    Area<override>() : float = Width * Height

square := class(shape):
    Side : float
    Area<override>() : float = Side * Side

TotalArea(Shapes : []shape) : float =
    var Total : float = 0.0
    for (S : Shapes):
        set Total += S.Area()
    Total
