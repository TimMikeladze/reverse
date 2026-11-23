# A Fortnite device, checked and run without UEFN. The device types are
# stand-ins built into this toolchain: same names as Epic's API, simple
# behaviour. Names starting with `Sim` exist only here, for tests.
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

# --- device code: this part is plain UEFN Verse ---

round_device := class(creative_device):
    @editable
    Button : button_device = button_device{}
    @editable
    Score : score_manager_device = score_manager_device{}
    @editable
    Clock : timer_device = timer_device{}
    @editable
    Hud : hud_message_device = hud_message_device{}
    @editable
    EndGame : end_game_device = end_game_device{}

    Leader<localizes>(Points : int) : message = "{Points} points wins the round"

    OnBegin<override>()<suspends> : void =
        Button.InteractedWithEvent.Subscribe(OnPressed)
        Clock.Start()
        # Wait for the timer, then find the best score.
        Clock.SuccessEvent.Await()
        Button.Disable()
        var Best : ?agent = false
        for (Player : GetPlayspace().GetPlayers()):
            if (not Best? or Score.GetCurrentScore(Player) > Score.GetCurrentScore(Best?)):
                set Best = option{Player}
        if (Winner := Best?):
            Hud.Show(Leader(Score.GetCurrentScore(Winner)))
            EndGame.Activate(Winner)

    OnPressed(Agent : agent) : void =
        Score.Activate(Agent)

# --- test code: keep this out of UEFN ---

# The clock is simulated, so the ten-second round finishes at once.
TestMostPressesWins()<suspends> : logic =
    Ana := SimAddPlayer("Ana")
    Ben := SimAddPlayer("Ben")
    Round := round_device{Clock := timer_device{SimDuration := 10.0}}
    SimStart(Round)
    Sleep(1.0)
    Round.Button.SimInteract(Ana)
    Round.Button.SimInteract(Ben)
    Round.Button.SimInteract(Ben)
    Sleep(10.0)
    Round.Button.SimInteract(Ana)        # too late: the button is disabled
    logic{SimWinner[] = Ben and SimScore(Ana) = 1 and Round.Hud.SimShown[0] = "2 points wins the round"}

Main()<suspends> : void =
    Passed := TestMostPressesWins()
    if (Passed?):
        Print("Ben won after {GetSimulationElapsedTime()} simulated seconds")
