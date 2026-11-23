# source: written for this project; type and member names from Epic's public Verse API reference (dev.epicgames.com/documentation/fortnite/verse-api), behaviour is this toolchain's simplified model
# uefn-verified: no
#> welcome Ana
#> welcome Ben
#> joined 2, in game 2
#> ana health 170.000000 shield 0.000000
#> healed to 200.000000 of 200.000000
#> invulnerable: 200.000000
#> Ana was eliminated
#> active: no
#> Ben was eliminated by Ana
#> Ben left
#> ben active: no, ana active: yes
#> participants 1
using { /Fortnite.com/Devices }
using { /Fortnite.com/Characters }
using { /Fortnite.com/Playspaces }
using { /Verse.org/Simulation }
using { /UnrealEngine.com/Temporary/Diagnostics }

arena_log := class(log_channel){}

arena := class(creative_device):
    Logger : log = log{Channel := arena_log}
    var Joined : int = 0

    OnBegin<override>()<suspends> : void =
        Playspace : fort_playspace = GetPlayspace()
        for (P : Playspace.GetPlayers()):
            Welcome(P)
        Playspace.PlayerAddedEvent().Subscribe(Welcome)
        Playspace.PlayerRemovedEvent().Subscribe(OnLeft)

    Welcome(P : player) : void =
        set Joined += 1
        Logger.Print("welcome {P.SimName}")
        if (C := P.GetFortCharacter[]):
            C.SetMaxHealth(200.0)
            C.SetHealth(200.0)
            C.SetShield(50.0)
            C.EliminatedEvent().Subscribe(OnEliminated)

    OnLeft(P : player) : void =
        Logger.Print("{P.SimName} left", ?Level := log_level.Warning)

    OnEliminated(Result : elimination_result) : void =
        Victim := Result.EliminatedCharacter
        if (A := Victim.GetAgent[]):
            if (By := Result.EliminatingCharacter?, Killer := By.GetAgent[]):
                Logger.Print("{A.SimName} was eliminated by {Killer.SimName}")
            else:
                Logger.Print("{A.SimName} was eliminated")

Main()<suspends> : void =
    Ana := SimAddPlayer("Ana")
    D := arena{}
    SimStart(D)
    Sleep(0.0)
    Ben := SimAddPlayer("Ben")
    Print("joined {D.Joined}, in game {D.GetPlayspace().GetPlayers().Length}")
    if (C := Ana.GetFortCharacter[]):
        C.Damage(80.0)
        Print("ana health {C.GetHealth()} shield {C.GetShield()}")
        C.Heal(500.0)
        Print("healed to {C.GetHealth()} of {C.GetMaxHealth()}")
        C.SetVulnerability(false)
        C.Damage(1000.0)
        Print("invulnerable: {C.GetHealth()}")
        C.SetVulnerability(true)
        C.Damage(1000.0)
        Print("active: {if (C.IsActive[]) then "yes" else "no"}")
    SimEliminate(Ben, option{Ana})
    SimRemovePlayer(Ben)
    Print("ben active: {if (Ben.IsActive[]) then "yes" else "no"}, ana active: {if (Ana.IsActive[]) then "yes" else "no"}")
    Print("participants {D.GetPlayspace().GetParticipants().Length}")
