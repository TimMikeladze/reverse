Write a Verse class `account` with a mutable `int` field `Balance` (default 0) and two methods:

- `Deposit(Amount : int) : void` adds `Amount` to the balance.
- `Withdraw(Amount : int)<decides><transacts> : void` subtracts `Amount`, and fails when
  that would make the balance negative. After a failed withdrawal the balance must be unchanged.

Also write `Transfer(From : account, To : account, Amount : int)<decides><transacts> : void`
that moves `Amount` between two accounts and fails, changing nothing, when `From` cannot cover it.
