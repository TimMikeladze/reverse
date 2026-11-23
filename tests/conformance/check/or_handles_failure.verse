# source: written for this project from the Book of Verse, Failure
# uefn-verified: no
# `A or B` cannot fail when B cannot fail, so it is allowed anywhere.
A(Xs : []int) : int = Xs[0] or Xs[1] or 0
# When the last alternative can fail, the whole expression can.
B(Xs : []int) : int = Xs[0] or Xs[1] #! V3001
