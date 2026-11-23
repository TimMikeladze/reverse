# source: written for this project; type and member names from Epic's public Verse API reference (dev.epicgames.com/documentation/fortnite/verse-api), behaviour is this toolchain's simplified model
# uefn-verified: no
#> left 6.000000
#> paused, left 6.000000
#> time up at 110.033333, started by Ana
#> time up at 112.033333, started by nobody
#> remaining 0
#> after reset 4
#> done at 174.033333
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

round := class(creative_device):
    @editable
    Start : trigger_device = trigger_device{}
    @editable
    Clock : timer_device = timer_device{}

    OnBegin<override>()<suspends> : void =
        Start.TriggeredEvent.Subscribe(OnStart)
        loop:
            Who := Clock.SuccessEvent.Await()
            if (A := Who?):
                Print("time up at {GetSimulationElapsedTime()}, started by {A.SimName}")
            else:
                Print("time up at {GetSimulationElapsedTime()}, started by nobody")

    OnStart(Who : ?agent) : void =
        if (A := Who?):
            Clock.Start(A)
        else:
            Clock.Start()

Main()<suspends> : void =
    Ana := SimAddPlayer("Ana")
    D := round{Clock := timer_device{SimDuration := 10.0}}
    SimStart(D)
    Sleep(0.0)
    D.Start.Trigger(Ana)
    Sleep(4.0)
    Print("left {D.Clock.GetActiveDuration()}")
    D.Clock.Pause()
    Sleep(100.0)
    Print("paused, left {D.Clock.GetActiveDuration()}")
    D.Clock.Resume()
    Sleep(7.0)
    # A second run, started by the trigger without an agent, then cut short.
    D.Start.Trigger()
    Sleep(1.0)
    D.Clock.Complete()
    Sleep(1.0)
    # Reset stops the clock without an event.
    D.Start.Trigger(Ana)
    Sleep(1.0)
    D.Clock.Reset()
    Sleep(60.0)
    # A trigger with a limit stops firing.
    D.Start.SetMaxTriggerCount(4)
    D.Start.Trigger(Ana)
    D.Start.Trigger(Ana)
    Print("remaining {D.Start.GetTriggerCountRemaining()}")
    D.Start.Reset()
    Print("after reset {D.Start.GetTriggerCountRemaining()}")
    Print("done at {GetSimulationElapsedTime()}")
