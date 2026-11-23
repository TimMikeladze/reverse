FizzBuzz(N : int) : string =
    if (Mod[N, 15] = 0):
        "FizzBuzz"
    else if (Mod[N, 3] = 0):
        "Fizz"
    else if (Mod[N, 5] = 0):
        "Buzz"
    else:
        "{N}"
