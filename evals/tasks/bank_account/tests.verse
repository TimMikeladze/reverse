TestDepositWithdraw()<decides><transacts> : void =
    A := account{}
    A.Deposit(50)
    A.Withdraw[20]
    A.Balance = 30
TestOverdraftFailsAndRollsBack()<decides><transacts> : void =
    A := account{Balance := 10}
    not A.Withdraw[11]
    A.Balance = 10
TestTransfer()<decides><transacts> : void =
    A := account{Balance := 100}
    B := account{}
    Transfer[A, B, 60]
    A.Balance = 40
    B.Balance = 60
TestFailedTransferChangesNothing()<decides><transacts> : void =
    A := account{Balance := 5}
    B := account{Balance := 1}
    not Transfer[A, B, 6]
    A.Balance = 5
    B.Balance = 1
