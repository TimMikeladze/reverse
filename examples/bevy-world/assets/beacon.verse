# source: written for this project from the Book of Verse, Failure, Concurrency, and Control Flow
# uefn-verified: no

BeaconProfile : string = "ORBIT"
PulseSeconds : float = 0.25

OnStart() : void =
    if (SetActive[false], SetLevel[0.0], SetLabel["{BeaconProfile} READY"]):
        Print("{BeaconProfile} firmware ready")

TryStart(Allowed : logic)<decides><transacts> : void =
    Allowed?
    not (GetActive[]?)
    SetLevel[0.0]
    SetActive[true]
    SetLabel["{BeaconProfile} SCANNING"]
    Report("{BeaconProfile} scan started")

CollectSpectrum()<suspends> : void =
    for (Fraction : array{0.25, 0.5, 0.75, 1.0}):
        Sleep(PulseSeconds)
        if (SetLevel[Fraction]) {}

BackupReceiver()<suspends> : string =
    defer:
        Print("Backup receiver released")
    Sleep(1.5)
    "backup"

FindSignal()<suspends> : void =
    Receiver := race:
        block:
            Sleep(0.375)
            "primary"
        BackupReceiver()
    Report("Signal acquired by {Receiver} receiver")

FinishScan()<decides><transacts> : int =
    Previous := GetUnits[]
    Next := Previous + 1
    SetUnits[Next]
    SetActive[false]
    SetLabel["{BeaconProfile} SCAN COMPLETE"]
    Report("{BeaconProfile} scan {Next} complete")
    Next

Archive(Scan : int)<suspends> : void =
    Sleep(0.5)
    Report("Scan {Scan} archived")

OnInteract(Allowed : logic)<suspends> : void =
    if (TryStart[Allowed]):
        sync:
            CollectSpectrum()
            FindSignal()
        if (Scan := FinishScan[]):
            spawn{Archive(Scan)}
