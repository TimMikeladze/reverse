# Fortnite devices

Most Verse written for UEFN talks to devices: a button, a timer, a score manager. Those
types live in Epic's API, which only UEFN knows. So that device code can still be checked,
run and tested here, the toolchain ships **stand-ins** for the most common ones.

A stand-in has the same name, methods and events as the real type, written from Epic's
[public API reference](https://dev.epicgames.com/documentation/fortnite/verse-api). Its
behaviour is a small, predictable model: a button fires its event when a test says a player
pressed it, a timer counts down on the simulated clock. It is not Epic's implementation, and
nothing in it comes from a UEFN digest file.

Stand-ins load by themselves when a file imports a `/Fortnite.com/...` module.

## A device and its test

Keep the device in one file and its tests in another. The device file is plain UEFN Verse
and goes into your project unchanged; the test file stays here.

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

counter_device := class(creative_device):
    @editable
    Button : button_device = button_device{}
    @editable
    Score : score_manager_device = score_manager_device{}

    OnBegin<override>()<suspends> : void =
        Button.InteractedWithEvent.Subscribe(OnPressed)

    OnPressed(Agent : agent) : void =
        Score.Activate(Agent)

TestPressScoresAPoint()<decides><transacts> : void =
    Ana := SimAddPlayer("Ana")
    Device := counter_device{}
    SimStart(Device)
    Device.Button.SimInteract(Ana)
    SimScore(Ana) = 1
```

```
reverse test counter.verse counter_test.verse
```

Files given together are checked as one program, and each diagnostic names its own file and
line. Over MCP, pass the test file in `files`. A complete example is in
[examples/door](../examples/door/door.verse).

## Rules

- **`Sim...` names are test-only.** `SimAddPlayer`, `Button.SimInteract`, `Timer.SimDuration`
  and every other name that starts with `Sim` exists only here. Using one outside `Main` or a
  function named `Test...` or `Sim...` is warning `V6007`, because that code would not
  compile in UEFN.
- **Only the game fires a device event.** Device events have type `listenable(t)`: you can
  `Subscribe` a handler or `Await()`, but `Signal` is error `V6005`. In a test, use the
  device's `Sim...` method instead.
- **Handlers are plain functions.** A handler takes the event's payload and returns `void`.
  It cannot be `<suspends>` or `<decides>`; start slow work inside it with `spawn{...}`.
  `Subscribe` returns a `cancelable`; call `Cancel()` on it to stop listening.
- **Every type needs its `using`.** `agent` and `player` come from `/Verse.org/Simulation`,
  `fort_character` from `/Fortnite.com/Characters`, and so on. A missing import is warning
  `V6006`, with a fix that adds the line.
- **What is not modelled is not checked.** A device or method the stand-ins do not cover
  gives warning `V6002` and is skipped, as before. A misspelt member of a modelled device is
  an error with a suggestion.
- **Your own definition wins.** If a file defines a type with the same name as a stand-in,
  the file's version is used.

## Driving the simulation

| Helper | What it does |
| --- | --- |
| `SimAddPlayer(Name)` | Adds a player with a character at full health; fires `PlayerAddedEvent`. Returns the `player`. |
| `SimRemovePlayer(Player)` | Removes the player; fires `PlayerRemovedEvent`. |
| `SimStart(Device)` | Starts the device's `OnBegin`, as the game does when a round begins. |
| `SimEnd(Device)` | Calls the device's `OnEnd`. |
| `SimEliminate(Victim, Eliminator)` | Eliminates the victim's character; `Eliminator` is `?agent`. |
| `SimScore(Agent)` | Points the score manager devices have awarded to the agent. |
| `SimGameEnded[]` | Succeeds once an end game device has ended the game. |
| `SimWinner[]` | The agent an end game device was activated for. |

`SimStart` runs `OnBegin` up to its first suspension straight away, so a `<decides>` test can
subscribe and press in the same breath. Use a `<suspends>` test when time has to pass; the
clock is simulated, so `Sleep(600.0)` returns at once. Each test starts with an empty world.

## Reference

Members marked *test helper* exist only in this toolchain. Everything else mirrors the
public API by name; members that are not listed are not modelled.

<!-- generated:reference -->
### `/Verse.org/Simulation`

**`agent`**

- *test helper* `SimName : string`: Name used in test output; `SimAddPlayer` sets it.

**`player`**

```
IsActive()<decides> : void
```

### `/Fortnite.com/Characters`

**`elimination_result`**

```
EliminatedCharacter : fort_character
EliminatingCharacter : ?fort_character
```

**`fort_character`**

```
GetAgent()<decides> : agent
GetHealth() : float
SetHealth(Health : float) : void
GetMaxHealth() : float
SetMaxHealth(MaxHealth : float) : void
GetShield() : float
SetShield(Shield : float) : void
GetMaxShield() : float
SetMaxShield(MaxShield : float) : void
Damage(Amount : float) : void
Heal(Amount : float) : void
EliminatedEvent() : listenable(elimination_result)
IsActive()<decides> : void
IsVulnerable()<decides> : void
SetVulnerability(Vulnerable : logic) : void
Show() : void
Hide() : void
```

- *test helper* `SimEliminate(By : ?fort_character) : void`: Eliminate this character, optionally crediting another one.
- *test helper* `SimRespawn() : void`: Bring the character back at full health.

### `/Fortnite.com/Playspaces`

**`fort_playspace`**

```
GetPlayers() : []player
GetParticipants() : []agent
PlayerAddedEvent() : listenable(player)
PlayerRemovedEvent() : listenable(player)
```

### `/UnrealEngine.com/Temporary/Diagnostics`

**`log_level`** (enum)

**`log_channel`** (class)

**`log`**

```
Channel : any
DefaultLevel : log_level
Print(Message : string, ?Level : log_level = log_level.Normal) : void
```

### `/Fortnite.com/Devices`

**`creative_object`**

```
GetPlayspace() : fort_playspace
```

- *test helper* `SimIsEnabled()<decides> : void`: Succeeds while the device is enabled.

**`creative_device_base`** (class)

**`creative_device`**

```
OnBegin()<suspends> : void
OnEnd() : void
```

**`button_device`**

```
InteractedWithEvent : listenable(agent)
Enable() : void
Disable() : void
SetInteractionText(Text : message) : void
SetInteractionTime(Time : float) : void
GetInteractionTime() : float
SetMaxTriggerCount(MaxCount : int) : void
GetMaxTriggerCount() : int
GetTriggerCountRemaining() : int
```

- *test helper* `SimInteract(Agent : agent) : void`: A player holds the interact key on the button.

**`trigger_device`**

```
TriggeredEvent : listenable(?agent)
Enable() : void
Disable() : void
Reset() : void
Trigger(Agent : agent) : void
Trigger() : void
SetMaxTriggerCount(MaxCount : int) : void
GetMaxTriggerCount() : int
GetTriggerCountRemaining() : int
SetResetDelay(Time : float) : void
GetResetDelay() : float
SetTransmitDelay(Time : float) : void
GetTransmitDelay() : float
```

- *test helper* `SimFire(Agent : ?agent) : void`: Fire the trigger as the game would; `Trigger` does the same from Verse.

**`timer_device`**

```
SuccessEvent : listenable(?agent)
FailureEvent : listenable(?agent)
StartUrgencyModeEvent : listenable(?agent)
Enable() : void
Enable(Agent : agent) : void
Disable() : void
Disable(Agent : agent) : void
Start() : void
Start(Agent : agent) : void
Pause() : void
Pause(Agent : agent) : void
Resume() : void
Resume(Agent : agent) : void
Reset() : void
Reset(Agent : agent) : void
Complete() : void
Complete(Agent : agent) : void
SetActiveDuration(Time : float) : void
SetActiveDuration(Time : float, Agent : agent) : void
GetActiveDuration() : float
GetActiveDuration(Agent : agent) : float
SetMaxDuration(Time : float) : void
GetMaxDuration() : float
```

- *test helper* `SimDuration : float`: Seconds the timer counts down from. Set it in the archetype in a test.
- *test helper* `SimSuccessOnEnd : logic`: The device's "Success on Timer End" option.

**`item_granter_device`**

```
ItemGrantedEvent : listenable(agent)
Enable() : void
Disable() : void
GrantItem(Agent : agent) : void
CycleToNextItem(Agent : agent) : void
CycleToPreviousItem(Agent : agent) : void
CycleToRandomItem(Agent : agent) : void
RestockItems() : void
```

- *test helper* `SimGrantCount(Agent : agent) : int`: How many times this device granted its item to the agent.

**`score_manager_device`**

```
ScoreOutputEvent : listenable(agent)
MaxTriggersEvent : listenable(agent)
Enable() : void
Enable(Agent : agent) : void
Disable() : void
Disable(Agent : agent) : void
Activate(Agent : agent) : void
Activate() : void
Increment() : void
Increment(Agent : agent) : void
Decrement() : void
Decrement(Agent : agent) : void
SetScoreAward(Score : int) : void
GetScoreAward() : int
GetCurrentScore(Agent : agent) : int
SetToAgentScore(Agent : agent) : void
```

- *test helper* `SimScoreAward : int`: Points awarded per activation. Set it in the archetype in a test.

**`hud_message_device`**

```
Show() : void
Show(Agent : agent) : void
Show(Agent : agent, Message : message, ?DisplayTime : float = 0.0) : void
Show(Message : message, ?DisplayTime : float = 0.0) : void
Hide() : void
Hide(Agent : agent) : void
SetText(Text : message) : void
SetDisplayTime(Time : float) : void
GetDisplayTime() : float
```

- *test helper* `SimShown : []string`: Every message shown, oldest first.

**`player_spawner_device`**

```
SpawnedEvent : listenable(agent)
Enable() : void
Disable() : void
```

- *test helper* `SimSpawn(Agent : agent) : void`: The game spawns an agent on this pad.

**`end_game_device`**

```
Enable() : void
Disable() : void
Activate(Agent : agent) : void
```

**`teleporter_device`**

```
EnterEvent : listenable(agent)
TeleportedEvent : listenable(agent)
Enable() : void
Disable() : void
IsEnabled()<decides> : void
Activate(Agent : agent) : void
Teleport(Agent : agent) : void
ActivateLinkToTarget() : void
DeactivateLinkToTarget() : void
ResetLinkToTarget() : void
```

- *test helper* `SimMoved : []agent`: Agents this device moved, oldest first.
- *test helper* `SimEnter(Agent : agent) : void`: A player walks into the teleporter.

**`mutator_zone_device`**

```
AgentEntersEvent : listenable(agent)
AgentExitsEvent : listenable(agent)
Enable() : void
Disable() : void
GetAgentsInVolume() : []agent
IsInVolume(Agent : agent)<decides> : void
```

- *test helper* `SimEnter(Agent : agent) : void`: An agent walks into the zone.
- *test helper* `SimExit(Agent : agent) : void`: An agent walks out of the zone.

**`elimination_manager_device`**

```
EliminatedEvent : listenable(agent)
EliminationEvent : listenable(?agent)
Enable() : void
Disable() : void
```

- *test helper* `SimEliminate(Victim : agent, Eliminator : ?agent) : void`: An elimination this device is set up to notice.
- *test helper* `SimEliminate(Victim : agent, Eliminator : ?agent, Apply : logic) : void`: The same, choosing whether the victim's character is really eliminated.

**`switch_device`**

```
TurnedOnEvent : listenable(agent)
TurnedOffEvent : listenable(agent)
Enable() : void
Disable() : void
TurnOn(Agent : agent) : void
TurnOff(Agent : agent) : void
ToggleState(Agent : agent) : void
GetCurrentState()<decides> : void
GetCurrentState(Agent : agent)<decides> : void
SetInteractionTime(Time : float) : void
GetInteractionTime() : float
```

- *test helper* `SimOn : logic`: Whether the switch is on.
- *test helper* `SimInteract(Agent : agent) : void`: A player flips the switch.

**`item_spawner_device`**

```
ItemPickedUpEvent : listenable(agent)
Enable() : void
Disable() : void
SpawnItem() : void
CycleToNextItem() : void
```

- *test helper* `SimPickUp(Agent : agent) : void`: A player picks up the spawned item.

**`billboard_device`**

```
ShowText() : void
HideText() : void
SetText(Text : message) : void
SetShowBorder(Show : logic) : void
GetShowBorder() : logic
SetTextSize(Size : int) : void
GetTextSize() : int
UpdateDisplay() : void
```

- *test helper* `SimText : string`: The text last set with `SetText`.

**`barrier_device`**

```
Enable() : void
Disable() : void
AddToIgnoreList(Agent : agent) : void
RemoveFromIgnoreList(Agent : agent) : void
RemoveAllFromIgnoreList() : void
```

- *test helper* `SimBlocks(Agent : agent)<decides> : void`: Succeeds when the barrier stops this agent.
<!-- /generated -->

## Limits

- The models are deliberately simple and **have not been compared with UEFN**. Where a
  device's real behaviour depends on options set in the editor, the stand-in picks one
  behaviour and says so in [Behaviour not yet verified](UNVERIFIED.md).
- Positions, transforms, tags, teams, classes, UI widgets, items and persistence are not
  modelled.
- Passing tests here means the logic holds against the model. Compile and play in UEFN
  before you ship.
- The stand-ins mirror public API names so that code is portable. They are original code
  under this project's licence. This arrangement needs a legal review before a public
  release, like the project's name.
