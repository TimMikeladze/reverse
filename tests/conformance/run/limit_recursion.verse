# source: written for this project from the Book of Verse, (resource limits are a property of this interpreter)
# uefn-verified: no
#= status: error:stack_overflow
Down(N : int) : int = Down(N + 1)
Main() : void = Print("{Down(0)}")
