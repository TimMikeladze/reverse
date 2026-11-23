TestPushPop()<decides><transacts> : void =
    S := stack{}
    S.Push(1)
    S.Push(2)
    S.Size() = 2
    S.Pop[] = 2
    S.Pop[] = 1
    S.Size() = 0
TestPopEmptyFails()<decides><transacts> : void =
    S := stack{}
    not S.Pop[]
    S.Size() = 0
