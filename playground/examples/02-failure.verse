# Failure is control flow, and a failed context rolls back its writes.

account := class:
    var Balance : int = 0
    var History : []string = array{}

    Withdraw(Amount : int)<decides><transacts> : void =
        set History += array{"withdrew {Amount}"}
        set Balance -= Amount
        Balance >= 0          # fails when overdrawn; both writes are undone

Main() : void =
    A := account{Balance := 50}
    if (A.Withdraw[80]):
        Print("unexpected")
    else:
        Print("80 refused: balance {A.Balance}, history {A.History.Length}")
    if (A.Withdraw[30]):
        Print("30 allowed: balance {A.Balance}, history {A.History.Length}")

    Prices : [string]int = map{"sword" => 30, "shield" => 45}
    Print("sword costs {Prices["sword"] or 0}")
    Print("bow costs {Prices["bow"] or 0}")
    Cheapest := option{Prices["dagger"]}
    Print(if (Cheapest?) then "found a dagger" else "no dagger for sale")
