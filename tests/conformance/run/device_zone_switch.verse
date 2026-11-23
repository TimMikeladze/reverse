# source: written for this project; type and member names from Epic's public Verse API reference (dev.epicgames.com/documentation/fortnite/verse-api), behaviour is this toolchain's simplified model
# uefn-verified: no
#> Ana entered, inside 1
#> Ben entered, inside 2
#> Ana exited
#> ben inside: yes
#> gate blocks ana: yes, sign closed
#> gate blocks ana: no, sign open
#> lever on: yes
#> gate blocks ana: yes, ben: no
#> Ana teleported
#> Ben teleported
#> grants ana 1 ben 1 moved 2
#> Ben down
#> credit Ana
#> Ana down
#> no credit
#> Ben spawned
#> ben active: yes
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

vault := class(creative_device):
    @editable
    Zone : mutator_zone_device = mutator_zone_device{}
    @editable
    Lever : switch_device = switch_device{}
    @editable
    Gate : barrier_device = barrier_device{}
    @editable
    Portal : teleporter_device = teleporter_device{}
    @editable
    Loot : item_spawner_device = item_spawner_device{}
    @editable
    Reward : item_granter_device = item_granter_device{}
    @editable
    Sign : billboard_device = billboard_device{}
    @editable
    Pad : player_spawner_device = player_spawner_device{}
    @editable
    Watch : elimination_manager_device = elimination_manager_device{}

    Label<localizes>(Text : string) : message = "{Text}"

    OnBegin<override>()<suspends> : void =
        Zone.AgentEntersEvent.Subscribe(OnEnter)
        Zone.AgentExitsEvent.Subscribe(OnExit)
        Lever.TurnedOnEvent.Subscribe(OnLeverOn)
        Lever.TurnedOffEvent.Subscribe(OnLeverOff)
        Loot.ItemPickedUpEvent.Subscribe(OnLoot)
        Portal.TeleportedEvent.Subscribe(OnArrived)
        Pad.SpawnedEvent.Subscribe(OnSpawned)
        Watch.EliminatedEvent.Subscribe(OnDown)
        Watch.EliminationEvent.Subscribe(OnCredit)
        Sign.SetText(Label("closed"))

    OnEnter(A : agent) : void =
        Print("{A.SimName} entered, inside {Zone.GetAgentsInVolume().Length}")
    OnExit(A : agent) : void =
        Print("{A.SimName} exited")
    OnLeverOn(A : agent) : void =
        Gate.Disable()
        Sign.SetText(Label("open"))
    OnLeverOff(A : agent) : void =
        Gate.Enable()
        Sign.SetText(Label("closed"))
    OnLoot(A : agent) : void =
        Reward.GrantItem(A)
        Portal.Teleport(A)
    OnArrived(A : agent) : void =
        Print("{A.SimName} teleported")
    OnSpawned(A : agent) : void =
        Print("{A.SimName} spawned")
    OnDown(A : agent) : void =
        Print("{A.SimName} down")
    OnCredit(A : ?agent) : void =
        if (Who := A?):
            Print("credit {Who.SimName}")
        else:
            Print("no credit")

Main()<suspends> : void =
    Ana := SimAddPlayer("Ana")
    Ben := SimAddPlayer("Ben")
    D := vault{}
    SimStart(D)
    Sleep(0.0)
    D.Zone.SimEnter(Ana)
    D.Zone.SimEnter(Ana)
    D.Zone.SimEnter(Ben)
    D.Zone.SimExit(Ana)
    Print("ben inside: {if (D.Zone.IsInVolume[Ben]) then "yes" else "no"}")
    Print("gate blocks ana: {if (D.Gate.SimBlocks[Ana]) then "yes" else "no"}, sign {D.Sign.SimText}")
    D.Lever.SimInteract(Ana)
    Print("gate blocks ana: {if (D.Gate.SimBlocks[Ana]) then "yes" else "no"}, sign {D.Sign.SimText}")
    Print("lever on: {if (D.Lever.GetCurrentState[]) then "yes" else "no"}")
    D.Lever.TurnOff(Ana)
    D.Gate.AddToIgnoreList(Ben)
    Print("gate blocks ana: {if (D.Gate.SimBlocks[Ana]) then "yes" else "no"}, ben: {if (D.Gate.SimBlocks[Ben]) then "yes" else "no"}")
    D.Loot.SimPickUp(Ana)
    D.Loot.SimPickUp(Ben)
    D.Loot.SpawnItem()
    D.Loot.SimPickUp(Ben)
    Print("grants ana {D.Reward.SimGrantCount(Ana)} ben {D.Reward.SimGrantCount(Ben)} moved {D.Portal.SimMoved.Length}")
    D.Watch.SimEliminate(Ben, option{Ana})
    D.Watch.SimEliminate(Ana, false)
    D.Pad.SimSpawn(Ben)
    Print("ben active: {if (Ben.IsActive[]) then "yes" else "no"}")
