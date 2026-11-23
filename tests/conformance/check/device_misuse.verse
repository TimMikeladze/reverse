# source: written for this project; type and member names from Epic's public Verse API reference (dev.epicgames.com/documentation/fortnite/verse-api)
# uefn-verified: no
using { /Fortnite.com/Devices }

# `agent` lives in /Verse.org/Simulation, which is not imported.
lobby := class(creative_device):
    @editable
    Button : button_device = button_device{}
    @editable
    Trigger : trigger_device = trigger_device{}
    @editable
    Clock : timer_device = timer_device{SimDuration := 5.0} #! V6007

    OnPressed(Agent : agent) : void = #! V6006
        Print("pressed")

    OnBegin<override>()<suspends> : void =
        Button.InteractedWithEvent.Subscribe(OnPressed)
        # A handler cannot suspend; spawn the slow part from a plain handler.
        Button.InteractedWithEvent.Subscribe(Slow) #! V2001
        # The trigger sends `?agent`, not `agent`.
        Trigger.TriggeredEvent.Subscribe(OnPressed) #! V2001
        # Only the game fires a device event.
        Button.InteractedWithEvent.Signal(agent{}) #! V6005
        # Simulation helpers do not exist in UEFN.
        Button.SimInteract(agent{}) #! V6007
        Clock.Start(1.5) #! V2001

    Slow(Agent : agent)<suspends> : void =
        Sleep(1.0)
