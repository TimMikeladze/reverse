# source: written for this project from the Book of Verse, Classes & Interfaces
# uefn-verified: no
shape := class<abstract>:
    Name : string
    Area() : float

circle := class(shape):
    Radius : float = 1.0
    Area<override>() : float = Radius * Radius

square := class(shape):
    Area() : float = 1.0 #! V7001

lonely := class:
    Area<override>() : float = 1.0 #! V7002

lazy := class(shape) {} #! V7004

A() : shape = shape{Name := "x"} #! V2011
B() : circle = circle{} #! V2005
C() : circle = circle{Name := "c", Radius := 2} #! V2001
D() : circle = circle{Name := "c", Raduis := 2.0} #! V1003
E() : circle = circle{Name := "c"}

point := struct:
    var X : int = 0 #! V5004
