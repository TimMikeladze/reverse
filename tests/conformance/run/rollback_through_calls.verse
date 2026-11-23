# source: written for this project from the Book of Verse, Failure (speculative execution)
# uefn-verified: no
#> insufficient: balance still 50, log length 0
#> ok: balance 20, log length 1
account := class:
    var Balance : int = 0
    var Log : []string = array{}

    Record(Entry : string)<transacts> : void =
        set Log += array{Entry}

    Withdraw(Amount : int)<decides><transacts> : void =
        Record("withdraw {Amount}")
        set Balance -= Amount
        Balance >= 0

Main() : void =
    A := account{Balance := 50}
    if (A.Withdraw[80]):
        Print("unexpected")
    else:
        Print("insufficient: balance still {A.Balance}, log length {A.Log.Length}")
    if (A.Withdraw[30]):
        Print("ok: balance {A.Balance}, log length {A.Log.Length}")
