# source: written for this project from the Book of Verse, Container types
# uefn-verified: no
#> entries a=1 b=2 c=3
#> last duplicate wins: 2, at the end: b a
#> missing key failed
#> merged x=1 y=20 z=30
#> updated 5
Main() : void =
    var M : [string]int = map{"a" => 1, "b" => 2}
    if (set M["c"] = 3) {}
    var Line : string = "entries"
    for (K -> V : M):
        set Line += " {K}={V}"
    Print(Line)
    D := map{"a" => 1, "b" => 0, "a" => 2}
    var Keys : string = ""
    for (K -> V : D):
        set Keys += " {K}"
    if (V := D["a"]):
        Print("last duplicate wins: {V}, at the end:{Keys}")
    if (not M["zzz"]):
        Print("missing key failed")
    Merged := ConcatenateMaps(map{"x" => 1, "y" => 2}, map{"y" => 20, "z" => 30})
    var Out : string = "merged"
    for (K -> V : Merged):
        set Out += " {K}={V}"
    Print(Out)
    if (set M["a"] += 4, V := M["a"]):
        Print("updated {V}")
