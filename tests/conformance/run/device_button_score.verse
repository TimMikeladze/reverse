# source: written for this project; type and member names from Epic's public Verse API reference (dev.epicgames.com/documentation/fortnite/verse-api), behaviour is this toolchain's simplified model
# uefn-verified: no
#> ana 10 ben 5 left 1
#> disabled: ana 10
#> ana 15 ended yes
#> winner is Ana
#> ben 5
#> bought, total 5
#> bought, total 5
#> bought, total 10
#> bought, total 15
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

shop := class(creative_device):
    @editable
    Button : button_device = button_device{}
    @editable
    Score : score_manager_device = score_manager_device{}
    @editable
    Hud : hud_message_device = hud_message_device{}
    @editable
    EndGame : end_game_device = end_game_device{}
    var Sub : ?cancelable = false

    Bought<localizes>(Total : int) : message = "bought, total {Total}"

    OnBegin<override>()<suspends> : void =
        Button.SetInteractionText(Bought(0))
        Button.SetMaxTriggerCount(4)
        Score.SetScoreAward(5)
        set Sub = option{Button.InteractedWithEvent.Subscribe(OnPressed)}

    OnPressed(Agent : agent) : void =
        Score.Activate(Agent)
        Total := Score.GetCurrentScore(Agent)
        Hud.Show(Agent, Bought(Total))
        if (Total >= 15):
            EndGame.Activate(Agent)
            if (S := Sub?):
                S.Cancel()

Main()<suspends> : void =
    Ana := SimAddPlayer("Ana")
    Ben := SimAddPlayer("Ben")
    D := shop{}
    SimStart(D)
    Sleep(0.0)
    D.Button.SimInteract(Ana)
    D.Button.SimInteract(Ben)
    D.Button.SimInteract(Ana)
    Print("ana {SimScore(Ana)} ben {SimScore(Ben)} left {D.Button.GetTriggerCountRemaining()}")
    D.Button.Disable()
    D.Button.SimInteract(Ana)
    Print("disabled: ana {SimScore(Ana)}")
    D.Button.Enable()
    D.Button.SimInteract(Ana)
    Print("ana {SimScore(Ana)} ended {if (SimGameEnded[]) then "yes" else "no"}")
    if (W := SimWinner[], W = Ana):
        Print("winner is Ana")
    # The limit of four presses is reached and the handler was cancelled.
    D.Button.SimInteract(Ben)
    Print("ben {SimScore(Ben)}")
    for (Line : D.Hud.SimShown):
        Print(Line)
