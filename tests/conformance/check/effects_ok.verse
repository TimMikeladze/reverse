# source: written for this project from the Book of Verse, Effects
# uefn-verified: no
Pure(X : int)<computes> : int = X * 2
Reader()<reads> : int = 1
Default() : void = Print("default is transacts")
Explicit()<transacts> : void = Print("same thing")
MayFail(X : int)<decides> : int = X > 0
MayFailAndWrite(X : int)<decides><transacts> : int =
    Print("writing")
    X > 0
Slow()<suspends> : void = Sleep(0.0)

# A pure function may call other pure functions.
PureCaller(X : int)<computes> : int = Pure(X) + 1
# A decides function may call a pure decides function.
Chain(X : int)<decides><computes> : int = MayFailPure[X]
MayFailPure(X : int)<decides><computes> : int = X > 10
