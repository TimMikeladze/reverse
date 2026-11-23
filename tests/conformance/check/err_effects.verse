# source: written for this project from the Book of Verse, Effects
# uefn-verified: no
A()<computes> : void = Print("no") #! V4002

B()<computes> : int =
    var X : int = 0 #! V4002
    set X = 1 #! V4002
    X #! V4002

C() : void = Sleep(1.0) #! V4001

D()<decides><suspends> : void = {} #! V4003
E()<computes><transacts> : void = {} #! V4003
F()<decides><decides> : int = 1 = 1 #! V4003
G()<decide> : int = 1 = 1 #! V4004
H<decides>() : int = 1 = 1 #! V4005
I()<public> : int = 1 #! V4005

J() : void =
    sync: #! V4001
        Slow()
        Slow()

Slow()<suspends> : void = Sleep(0.0)
