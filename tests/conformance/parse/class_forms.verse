# source: written for this project from the Book of Verse, Classes & Interfaces, Access Specifiers
# uefn-verified: no
base := class<abstract>:
    Name<public> : string
    var Count<private> : int = 0
    Describe<public>() : string
    Helper<protected>()<computes> : int = 1

child := class<final>(base):
    @editable
    Extra : float = 1.0
    Describe<override>() : string = "{Name}"

brace_style := class {
    A : int = 0
    B : int = 0
}

empty := class {}
