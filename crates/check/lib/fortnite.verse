# Stand-ins for common Fortnite API types.
#
# Written for this toolchain from Epic's public Verse API reference: the type,
# method and event names match the reference so device code checks and runs
# here unchanged. The behaviour is a simplified model, not Epic's. Nothing in
# this file comes from a UEFN digest file.
#
# Names that start with `Sim` do not exist in UEFN. They drive the simulation
# from tests (press a button, add a player) and expose what a device did.
#
# A line `#@ <module path>` names the module the following definitions belong
# to in UEFN; `#@ sim` marks test-only helpers.

#@ /Verse.org/Simulation
agent<public> := class<unique>:
    # Name used in test output; `SimAddPlayer` sets it.
    SimName<public> : string = "agent"
    var SimCharacter<public> : ?fort_character = false

player<public> := class<unique>(agent):
    IsActive<public>()<transacts><decides> : void =
        SimPlayspace.SimHas[Self]

#@ /Fortnite.com/Characters
elimination_result<public> := struct:
    EliminatedCharacter<public> : fort_character
    EliminatingCharacter<public> : ?fort_character = false

fort_character<public> := class<unique>:
    SimOwner<public> : agent
    var SimHealth<public> : float = 100.0
    var SimMaxHealth<public> : float = 100.0
    var SimShield<public> : float = 0.0
    var SimMaxShield<public> : float = 100.0
    var SimActive<public> : logic = true
    var SimVulnerable<public> : logic = true
    var SimVisible<public> : logic = true
    SimEliminated<public> : event(elimination_result) = event(elimination_result){}

    GetAgent<public>()<transacts><decides> : agent =
        SimOwner.SimCharacter?
        SimOwner
    GetHealth<public>()<transacts> : float = SimHealth
    SetHealth<public>(Health : float) : void =
        set SimHealth = Min(SimMaxHealth, Max(1.0, Health))
    GetMaxHealth<public>()<transacts> : float = SimMaxHealth
    SetMaxHealth<public>(MaxHealth : float) : void =
        set SimMaxHealth = Max(1.0, MaxHealth)
        set SimHealth = Min(SimHealth, SimMaxHealth)
    GetShield<public>()<transacts> : float = SimShield
    SetShield<public>(Shield : float) : void =
        set SimShield = Min(SimMaxShield, Max(0.0, Shield))
    GetMaxShield<public>()<transacts> : float = SimMaxShield
    SetMaxShield<public>(MaxShield : float) : void =
        set SimMaxShield = Max(0.0, MaxShield)
        set SimShield = Min(SimShield, SimMaxShield)
    Damage<public>(Amount : float) : void =
        if (SimActive?, SimVulnerable?, Amount > 0.0):
            Absorbed := Min(SimShield, Amount)
            set SimShield = SimShield - Absorbed
            set SimHealth = Max(0.0, SimHealth - (Amount - Absorbed))
            if (SimHealth <= 0.0):
                SimEliminate(false)
    Heal<public>(Amount : float) : void =
        if (SimActive?, Amount > 0.0):
            set SimHealth = Min(SimMaxHealth, SimHealth + Amount)
    EliminatedEvent<public>() : listenable(elimination_result) = SimEliminated
    IsActive<public>()<transacts><decides> : void =
        SimActive?
    IsVulnerable<public>()<transacts><decides> : void =
        SimVulnerable?
    SetVulnerability<public>(Vulnerable : logic) : void =
        set SimVulnerable = Vulnerable
    Show<public>() : void =
        set SimVisible = true
    Hide<public>() : void =
        set SimVisible = false
    # Eliminate this character, optionally crediting another one.
    SimEliminate<public>(By : ?fort_character) : void =
        if (SimActive?):
            set SimActive = false
            set SimHealth = 0.0
            SimEliminated.Signal(elimination_result{EliminatedCharacter := Self, EliminatingCharacter := By})
    # Bring the character back at full health.
    SimRespawn<public>() : void =
        set SimActive = true
        set SimHealth = SimMaxHealth
        set SimShield = 0.0

(InAgent : agent).GetFortCharacter<public>()<transacts><decides> : fort_character =
    InAgent.SimCharacter?

#@ /Fortnite.com/Playspaces
fort_playspace<public> := class:
    var SimPlayers<public> : []player = array{}
    var SimScores<public> : [agent]int = map{}
    var SimEnded<public> : logic = false
    var SimWinner<public> : ?agent = false
    SimPlayerAdded<public> : event(player) = event(player){}
    SimPlayerRemoved<public> : event(player) = event(player){}

    GetPlayers<public>()<transacts> : []player = SimPlayers
    GetParticipants<public>()<transacts> : []agent =
        for (P : SimPlayers) { P }
    PlayerAddedEvent<public>() : listenable(player) = SimPlayerAdded
    PlayerRemovedEvent<public>() : listenable(player) = SimPlayerRemoved
    SimHas<public>(P : player)<transacts><decides> : void =
        Found := for (X : SimPlayers, X = P) { X }
        Found.Length > 0

#@ sim
SimPlayspace<public> : fort_playspace = fort_playspace{}

# Add a player (with a character at full health) and announce it.
SimAddPlayer<public>(Name : string) : player =
    P := player{SimName := Name}
    set P.SimCharacter = option{fort_character{SimOwner := P}}
    set SimPlayspace.SimPlayers += array{P}
    SimPlayspace.SimPlayerAdded.Signal(P)
    P

SimRemovePlayer<public>(P : player) : void =
    set SimPlayspace.SimPlayers = for (X : SimPlayspace.SimPlayers, X <> P) { X }
    SimPlayspace.SimPlayerRemoved.Signal(P)

# Start a device the way the game does when a round begins.
SimStart<public>(Device : creative_device) : void =
    spawn{Device.OnBegin()}

SimEnd<public>(Device : creative_device) : void =
    Device.OnEnd()

# Eliminate an agent's character, optionally crediting another agent.
SimEliminate<public>(Victim : agent, Eliminator : ?agent) : void =
    if (C := Victim.SimCharacter?):
        if (E := Eliminator?, EC := E.SimCharacter?):
            C.SimEliminate(option{EC})
        else:
            C.SimEliminate(false)

# Total score the score manager devices have awarded to an agent.
SimScore<public>(Agent : agent)<transacts> : int =
    if (S := SimPlayspace.SimScores[Agent]) then S else 0

# Succeeds once an end game device has ended the game.
SimGameEnded<public>()<transacts><decides> : void =
    SimPlayspace.SimEnded?

SimWinner<public>()<transacts><decides> : agent =
    SimPlayspace.SimWinner?

SimPrintLine<public>(Line : string) : void =
    Print(Line)

#@ /UnrealEngine.com/Temporary/Diagnostics
log_level<public> := enum:
    Debug
    Verbose
    Normal
    Warning
    Error

log_channel<public> := class<abstract>:

log<public> := class:
    Channel<public> : any
    DefaultLevel<public> : log_level = log_level.Normal
    Print<public>(Message : string, ?Level : log_level = log_level.Normal) : void =
        SimPrintLine(Message)

#@ /Fortnite.com/Devices
creative_object<public> := class:
    var SimEnabled<public> : logic = true
    GetPlayspace<public>()<transacts> : fort_playspace = SimPlayspace
    # Succeeds while the device is enabled.
    SimIsEnabled<public>()<transacts><decides> : void =
        SimEnabled?

creative_device_base<public> := class(creative_object):

creative_device<public> := class(creative_object):
    OnBegin<public>()<suspends> : void = {}
    OnEnd<public>() : void = {}

button_device<public> := class(creative_device_base):
    InteractedWithEvent<public> : listenable(agent) = event(agent){}
    var SimMaxTriggerCount<public> : int = 0
    var SimTriggerCount<public> : int = 0
    var SimInteractionTime<public> : float = 0.0
    var SimInteractionText<public> : string = ""

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    SetInteractionText<public>(Text : message) : void =
        set SimInteractionText = Text
    SetInteractionTime<public>(Time : float) : void =
        set SimInteractionTime = Max(0.0, Time)
    GetInteractionTime<public>()<transacts> : float = SimInteractionTime
    SetMaxTriggerCount<public>(MaxCount : int) : void =
        set SimMaxTriggerCount = Max(0, MaxCount)
    GetMaxTriggerCount<public>()<transacts> : int = SimMaxTriggerCount
    GetTriggerCountRemaining<public>()<transacts> : int =
        Max(0, SimMaxTriggerCount - SimTriggerCount)
    # A player holds the interact key on the button.
    SimInteract<public>(Agent : agent) : void =
        if (SimEnabled?, SimMaxTriggerCount = 0 or SimTriggerCount < SimMaxTriggerCount):
            set SimTriggerCount += 1
            InteractedWithEvent.Signal(Agent)

trigger_device<public> := class(creative_device_base):
    TriggeredEvent<public> : listenable(?agent) = event(?agent){}
    var SimMaxTriggerCount<public> : int = 0
    var SimTriggerCount<public> : int = 0
    var SimResetDelay<public> : float = 0.0
    var SimTransmitDelay<public> : float = 0.0

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    Reset<public>() : void =
        set SimTriggerCount = 0
    Trigger<public>(Agent : agent) : void =
        SimFire(option{Agent})
    Trigger<public>() : void =
        SimFire(false)
    SetMaxTriggerCount<public>(MaxCount : int) : void =
        set SimMaxTriggerCount = Max(0, MaxCount)
    GetMaxTriggerCount<public>()<transacts> : int = SimMaxTriggerCount
    GetTriggerCountRemaining<public>()<transacts> : int =
        Max(0, SimMaxTriggerCount - SimTriggerCount)
    SetResetDelay<public>(Time : float) : void =
        set SimResetDelay = Max(0.0, Time)
    GetResetDelay<public>()<transacts> : float = SimResetDelay
    SetTransmitDelay<public>(Time : float) : void =
        set SimTransmitDelay = Max(0.0, Time)
    GetTransmitDelay<public>()<transacts> : float = SimTransmitDelay
    # Fire the trigger as the game would; `Trigger` does the same from Verse.
    SimFire<public>(Agent : ?agent) : void =
        if (SimEnabled?, SimMaxTriggerCount = 0 or SimTriggerCount < SimMaxTriggerCount):
            set SimTriggerCount += 1
            TriggeredEvent.Signal(Agent)

timer_device<public> := class(creative_device_base):
    SuccessEvent<public> : listenable(?agent) = event(?agent){}
    FailureEvent<public> : listenable(?agent) = event(?agent){}
    StartUrgencyModeEvent<public> : listenable(?agent) = event(?agent){}
    # Seconds the timer counts down from. Set it in the archetype in a test.
    var SimDuration<public> : float = 60.0
    # The device's "Success on Timer End" option.
    SimSuccessOnEnd<public> : logic = true
    var SimRemaining<public> : float = -1.0
    var SimEndsAt<public> : float = 0.0
    var SimRunning<public> : logic = false
    var SimRun<public> : int = 0
    var SimAgent<public> : ?agent = false

    Enable<public>() : void =
        set SimEnabled = true
    Enable<public>(Agent : agent) : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    Disable<public>(Agent : agent) : void =
        set SimEnabled = false
    Start<public>() : void =
        SimBegin(false)
    Start<public>(Agent : agent) : void =
        SimBegin(option{Agent})
    Pause<public>() : void =
        if (SimRunning?):
            set SimRemaining = Max(0.0, SimEndsAt - GetSimulationElapsedTime())
            set SimRunning = false
            set SimRun += 1
    Pause<public>(Agent : agent) : void =
        Pause()
    Resume<public>() : void =
        if (not SimRunning?, SimRemaining >= 0.0):
            SimBegin(SimAgent)
    Resume<public>(Agent : agent) : void =
        Resume()
    Reset<public>() : void =
        set SimRunning = false
        set SimRemaining = -1.0
        set SimRun += 1
    Reset<public>(Agent : agent) : void =
        Reset()
    Complete<public>() : void =
        if (SimRunning?):
            SimFinish()
    Complete<public>(Agent : agent) : void =
        Complete()
    SetActiveDuration<public>(Time : float) : void =
        set SimRemaining = Max(0.0, Time)
        if (SimRunning?):
            SimBegin(SimAgent)
    SetActiveDuration<public>(Time : float, Agent : agent) : void =
        SetActiveDuration(Time)
    GetActiveDuration<public>()<transacts> : float =
        if (SimRunning?) then Max(0.0, SimEndsAt - GetSimulationElapsedTime()) else if (SimRemaining >= 0.0) then SimRemaining else SimDuration
    GetActiveDuration<public>(Agent : agent)<transacts> : float =
        GetActiveDuration()
    SetMaxDuration<public>(Time : float) : void =
        set SimDuration = Max(0.0, Time)
    GetMaxDuration<public>()<transacts> : float = SimDuration
    SimBegin<public>(Agent : ?agent) : void =
        if (SimEnabled?):
            Left := if (SimRemaining >= 0.0) then SimRemaining else SimDuration
            set SimAgent = Agent
            set SimRemaining = Left
            set SimEndsAt = GetSimulationElapsedTime() + Left
            set SimRunning = true
            set SimRun += 1
            spawn{SimCountDown(SimRun, Left)}
    SimCountDown<public>(Run : int, Left : float)<suspends> : void =
        Sleep(Left)
        if (Run = SimRun, SimRunning?):
            SimFinish()
    SimFinish<public>() : void =
        set SimRunning = false
        set SimRemaining = -1.0
        set SimRun += 1
        if (SimSuccessOnEnd?):
            SuccessEvent.Signal(SimAgent)
        else:
            FailureEvent.Signal(SimAgent)

item_granter_device<public> := class(creative_device_base):
    ItemGrantedEvent<public> : listenable(agent) = event(agent){}
    var SimGrants<public> : [agent]int = map{}
    var SimItemIndex<public> : int = 0

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    GrantItem<public>(Agent : agent) : void =
        if (SimEnabled?):
            N := SimGrantCount(Agent)
            if (set SimGrants[Agent] = N + 1) {}
            ItemGrantedEvent.Signal(Agent)
    CycleToNextItem<public>(Agent : agent) : void =
        set SimItemIndex += 1
    CycleToPreviousItem<public>(Agent : agent) : void =
        set SimItemIndex = Max(0, SimItemIndex - 1)
    CycleToRandomItem<public>(Agent : agent) : void =
        set SimItemIndex = 0
    RestockItems<public>() : void = {}
    # How many times this device granted its item to the agent.
    SimGrantCount<public>(Agent : agent)<transacts> : int =
        if (N := SimGrants[Agent]) then N else 0

score_manager_device<public> := class(creative_device_base):
    ScoreOutputEvent<public> : listenable(agent) = event(agent){}
    MaxTriggersEvent<public> : listenable(agent) = event(agent){}
    # Points awarded per activation. Set it in the archetype in a test.
    var SimScoreAward<public> : int = 1

    Enable<public>() : void =
        set SimEnabled = true
    Enable<public>(Agent : agent) : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    Disable<public>(Agent : agent) : void =
        set SimEnabled = false
    Activate<public>(Agent : agent) : void =
        if (SimEnabled?):
            Total := SimScore(Agent) + SimScoreAward
            if (set SimPlayspace.SimScores[Agent] = Total) {}
            ScoreOutputEvent.Signal(Agent)
    Activate<public>() : void =
        for (P : SimPlayspace.SimPlayers):
            Activate(P)
    Increment<public>() : void =
        set SimScoreAward += 1
    Increment<public>(Agent : agent) : void =
        set SimScoreAward += 1
    Decrement<public>() : void =
        set SimScoreAward -= 1
    Decrement<public>(Agent : agent) : void =
        set SimScoreAward -= 1
    SetScoreAward<public>(Score : int) : void =
        set SimScoreAward = Score
    GetScoreAward<public>()<transacts> : int = SimScoreAward
    GetCurrentScore<public>(Agent : agent)<transacts> : int = SimScore(Agent)
    SetToAgentScore<public>(Agent : agent) : void =
        set SimScoreAward = SimScore(Agent)

hud_message_device<public> := class(creative_device_base):
    var SimText<public> : string = ""
    var SimDisplayTime<public> : float = 0.0
    # Every message shown, oldest first.
    var SimShown<public> : []string = array{}
    var SimVisible<public> : logic = false

    Show<public>() : void =
        SimShow(SimText)
    Show<public>(Agent : agent) : void =
        SimShow(SimText)
    Show<public>(Agent : agent, Message : message, ?DisplayTime : float = 0.0) : void =
        SimShow(Message)
    Show<public>(Message : message, ?DisplayTime : float = 0.0) : void =
        SimShow(Message)
    Hide<public>() : void =
        set SimVisible = false
    Hide<public>(Agent : agent) : void =
        set SimVisible = false
    SetText<public>(Text : message) : void =
        set SimText = Text
    SetDisplayTime<public>(Time : float) : void =
        set SimDisplayTime = Max(0.0, Time)
    GetDisplayTime<public>()<transacts> : float = SimDisplayTime
    SimShow<public>(Text : string) : void =
        set SimVisible = true
        set SimShown += array{Text}

player_spawner_device<public> := class(creative_device_base):
    SpawnedEvent<public> : listenable(agent) = event(agent){}

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    # The game spawns an agent on this pad.
    SimSpawn<public>(Agent : agent) : void =
        if (SimEnabled?):
            if (C := Agent.SimCharacter?):
                C.SimRespawn()
            SpawnedEvent.Signal(Agent)

end_game_device<public> := class(creative_device_base):
    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    Activate<public>(Agent : agent) : void =
        if (SimEnabled?, not SimPlayspace.SimEnded?):
            set SimPlayspace.SimEnded = true
            set SimPlayspace.SimWinner = option{Agent}

teleporter_device<public> := class(creative_device_base):
    EnterEvent<public> : listenable(agent) = event(agent){}
    TeleportedEvent<public> : listenable(agent) = event(agent){}
    var SimLinked<public> : logic = true
    # Agents this device moved, oldest first.
    var SimMoved<public> : []agent = array{}

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    IsEnabled<public>()<transacts><decides> : void =
        SimEnabled?
    Activate<public>(Agent : agent) : void =
        if (SimEnabled?, SimLinked?):
            set SimMoved += array{Agent}
            EnterEvent.Signal(Agent)
    Teleport<public>(Agent : agent) : void =
        if (SimEnabled?):
            set SimMoved += array{Agent}
            TeleportedEvent.Signal(Agent)
    ActivateLinkToTarget<public>() : void =
        set SimLinked = true
    DeactivateLinkToTarget<public>() : void =
        set SimLinked = false
    ResetLinkToTarget<public>() : void =
        set SimLinked = true
    # A player walks into the teleporter.
    SimEnter<public>(Agent : agent) : void =
        Activate(Agent)

mutator_zone_device<public> := class(creative_device_base):
    AgentEntersEvent<public> : listenable(agent) = event(agent){}
    AgentExitsEvent<public> : listenable(agent) = event(agent){}
    var SimInside<public> : []agent = array{}

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    GetAgentsInVolume<public>()<transacts> : []agent = SimInside
    IsInVolume<public>(Agent : agent)<transacts><decides> : void =
        Found := for (X : SimInside, X = Agent) { X }
        Found.Length > 0
    # An agent walks into the zone.
    SimEnter<public>(Agent : agent) : void =
        if (SimEnabled?, not IsInVolume[Agent]):
            set SimInside += array{Agent}
            AgentEntersEvent.Signal(Agent)
    # An agent walks out of the zone.
    SimExit<public>(Agent : agent) : void =
        if (IsInVolume[Agent]):
            set SimInside = for (X : SimInside, X <> Agent) { X }
            AgentExitsEvent.Signal(Agent)

elimination_manager_device<public> := class(creative_device_base):
    EliminatedEvent<public> : listenable(agent) = event(agent){}
    EliminationEvent<public> : listenable(?agent) = event(?agent){}

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    # An elimination this device is set up to notice.
    SimEliminate<public>(Victim : agent, Eliminator : ?agent) : void =
        SimEliminate(Victim, Eliminator, true)
    # The same, choosing whether the victim's character is really eliminated.
    SimEliminate<public>(Victim : agent, Eliminator : ?agent, Apply : logic) : void =
        if (Apply?, C := Victim.SimCharacter?):
            if (E := Eliminator?, EC := E.SimCharacter?):
                C.SimEliminate(option{EC})
            else:
                C.SimEliminate(false)
        if (SimEnabled?):
            EliminatedEvent.Signal(Victim)
            EliminationEvent.Signal(Eliminator)

switch_device<public> := class(creative_device_base):
    TurnedOnEvent<public> : listenable(agent) = event(agent){}
    TurnedOffEvent<public> : listenable(agent) = event(agent){}
    # Whether the switch is on.
    var SimOn<public> : logic = false
    var SimInteractionTime<public> : float = 0.0

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    TurnOn<public>(Agent : agent) : void =
        if (not SimOn?):
            set SimOn = true
            TurnedOnEvent.Signal(Agent)
    TurnOff<public>(Agent : agent) : void =
        if (SimOn?):
            set SimOn = false
            TurnedOffEvent.Signal(Agent)
    ToggleState<public>(Agent : agent) : void =
        if (SimOn?):
            TurnOff(Agent)
        else:
            TurnOn(Agent)
    GetCurrentState<public>()<transacts><decides> : void =
        SimOn?
    GetCurrentState<public>(Agent : agent)<transacts><decides> : void =
        SimOn?
    SetInteractionTime<public>(Time : float) : void =
        set SimInteractionTime = Max(0.0, Time)
    GetInteractionTime<public>()<transacts> : float = SimInteractionTime
    # A player flips the switch.
    SimInteract<public>(Agent : agent) : void =
        if (SimEnabled?):
            ToggleState(Agent)

item_spawner_device<public> := class(creative_device_base):
    ItemPickedUpEvent<public> : listenable(agent) = event(agent){}
    var SimItemReady<public> : logic = true
    var SimItemIndex<public> : int = 0

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    SpawnItem<public>() : void =
        if (SimEnabled?):
            set SimItemReady = true
    CycleToNextItem<public>() : void =
        set SimItemIndex += 1
    # A player picks up the spawned item.
    SimPickUp<public>(Agent : agent) : void =
        if (SimEnabled?, SimItemReady?):
            set SimItemReady = false
            ItemPickedUpEvent.Signal(Agent)

billboard_device<public> := class(creative_device_base):
    # The text last set with `SetText`.
    var SimText<public> : string = ""
    var SimTextVisible<public> : logic = true
    var SimBorder<public> : logic = true
    var SimTextSize<public> : int = 12

    ShowText<public>() : void =
        set SimTextVisible = true
    HideText<public>() : void =
        set SimTextVisible = false
    SetText<public>(Text : message) : void =
        set SimText = Text
    SetShowBorder<public>(Show : logic) : void =
        set SimBorder = Show
    GetShowBorder<public>()<transacts> : logic = SimBorder
    SetTextSize<public>(Size : int) : void =
        set SimTextSize = Min(24, Max(8, Size))
    GetTextSize<public>()<transacts> : int = SimTextSize
    UpdateDisplay<public>() : void = {}

barrier_device<public> := class(creative_device_base):
    var SimIgnored<public> : []agent = array{}

    Enable<public>() : void =
        set SimEnabled = true
    Disable<public>() : void =
        set SimEnabled = false
    AddToIgnoreList<public>(Agent : agent) : void =
        if (not SimIgnores[Agent]):
            set SimIgnored += array{Agent}
    RemoveFromIgnoreList<public>(Agent : agent) : void =
        set SimIgnored = for (X : SimIgnored, X <> Agent) { X }
    RemoveAllFromIgnoreList<public>() : void =
        set SimIgnored = array{}
    SimIgnores<public>(Agent : agent)<transacts><decides> : void =
        Found := for (X : SimIgnored, X = Agent) { X }
        Found.Length > 0
    # Succeeds when the barrier stops this agent.
    SimBlocks<public>(Agent : agent)<transacts><decides> : void =
        SimEnabled?
        not SimIgnores[Agent]
