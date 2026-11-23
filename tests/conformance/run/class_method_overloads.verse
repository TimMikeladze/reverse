# source: written for this project from the Book of Verse, Functions (overloading); the public device API overloads methods the same way (for example hud_message_device.Show)
# uefn-verified: no
#> all
#> zed
#> zed:hello
#> zed:timed
#> everyone
#> has zed
agent := class<unique>:
    Name : string = "a"

res := struct:
    Who : agent
    By : ?agent = false

hud := class:
    var Log : []string = array{}
    var Counts : [agent]int = map{}
    Show() : void =
        set Log += array{"all"}
    Show(A : agent) : void =
        set Log += array{A.Name}
        N := if (C := Counts[A]) then C else 0
        if (set Counts[A] = N + 1) {}
    Show(A : agent, M : string, ?DisplayTime : float = 0.0) : void =
        set Log += array{"{A.Name}:{M}"}
    Show(M : string) : void =
        set Log += array{M}
    Me() : hud = Self
    Has(A : agent)<transacts><decides> : void =
        Counts[A] > 0

Main() : void =
    H := hud{}
    Z := agent{Name := "zed"}
    H.Show()
    H.Show(Z)
    H.Show(Z, "hello")
    H.Show(Z, "timed", ?DisplayTime := 2.0)
    H.Show("everyone")
    R := res{Who := Z}
    for (L : H.Me().Log):
        Print(L)
    if (H.Has[Z]):
        Print("has {R.Who.Name}")
