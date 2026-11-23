# source: written for this project from the Book of Verse, Primitive types and Container types
# uefn-verified: no
I1 : int = 42
I2 : int = 0xFF
I3 : int = -7
F1 : float = 1.0
F2 : float = 2.5e3
F3 : float = 1.5e-2
C1 : char = 'a'
C2 : char = '\n'
S1 : string = "plain"
S2 : string = "tab\tquote\"brace\{\}"
S3 : string = "interp {I1} and {I1 + I2}"
A1 : []int = array{1, 2, 3}
A2 : []int = array{}
M1 : [string]int = map{"a" => 1, "b" => 2}
T1 : tuple(int, string) = (1, "x")
O1 : ?int = option{1}
O2 : ?int = false
L1 : logic = true
