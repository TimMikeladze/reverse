# A door that opens after three button presses and ends the round.
# This file is plain UEFN Verse: paste it into a project unchanged.
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

door_device := class(creative_device):
    @editable
    Button : button_device = button_device{}
    @editable
    Score : score_manager_device = score_manager_device{}
    @editable
    Hud : hud_message_device = hud_message_device{}
    @editable
    Clock : timer_device = timer_device{}
    @editable
    EndGame : end_game_device = end_game_device{}

    var Presses : [agent]int = map{}
    var Open : logic = false

    Progress<localizes>(Count : int) : message = "{Count} of 3"
    TooSlow<localizes> : message = "Too slow, the door stays shut"

    OnBegin<override>()<suspends> : void =
        Button.InteractedWithEvent.Subscribe(OnPressed)
        Clock.Start()
        # Whoever is slower loses: the door opening, or the clock running out.
        race:
            block:
                Clock.SuccessEvent.Await()
                if (not Open?):
                    Button.Disable()
                    Hud.Show(TooSlow)
            loop:
                Sleep(0.5)
                if (Open?):
                    break

    OnPressed(Agent : agent) : void =
        Count := (if (Old := Presses[Agent]) then Old else 0) + 1
        if (set Presses[Agent] = Count) {}
        Score.Activate(Agent)
        Hud.Show(Agent, Progress(Count))
        if (Count >= 3):
            set Open = true
            Clock.Pause()
            EndGame.Activate(Agent)
