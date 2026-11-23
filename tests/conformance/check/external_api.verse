# source: written for this project from the Book of Verse, Modules and Paths (digest files are never bundled)
# uefn-verified: no
using { /Fortnite.com/Devices }
using { /Fortnite.com/AI }
using { /Verse.org/Simulation }

# Types the built-in stand-ins do not model come from digests that are not
# loaded: uses are warned about, not rejected.
my_guard := class(npc_behavior): #! V6002
    @editable
    Mover : prop_mover_device = prop_mover_device{} #! V6002

    OnBegin<override>()<suspends> : void =
        Print("started")

# A modelled type is checked properly: a missing member is a warning, a
# misspelt one is an error with a suggestion.
my_device := class(creative_device):
    @editable
    Button : button_device = button_device{}

    OnBegin<override>()<suspends> : void =
        Button.SetCollisionProfile() #! V6002
        Button.Enabel() #! V1003
