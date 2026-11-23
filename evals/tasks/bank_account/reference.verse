account := class:
    var Balance : int = 0

    Deposit(Amount : int) : void =
        set Balance += Amount

    Withdraw(Amount : int)<decides><transacts> : void =
        set Balance -= Amount
        Balance >= 0

Transfer(From : account, To : account, Amount : int)<decides><transacts> : void =
    To.Deposit(Amount)
    From.Withdraw[Amount]
