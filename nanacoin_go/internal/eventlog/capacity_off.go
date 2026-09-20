//go:build nanacoin_nologs

package eventlog

// Capacity is zero in a no-logs build.
//
// A boolean would not have been enough. The ring is an array *inside* Log,
// so [58]Event and [58]string occupy their space whether or not a flag says
// to write to them - and on this target the allocation happens at boot or it
// does not happen at all. Making the capacity itself the compile-time
// constant is what actually reclaims the RAM: the arrays become zero-length,
// and the methods that walk them fold away to nothing.
//
// Log's methods stay defined and stay safe to call. Every caller already
// tolerates a nil *Log, and a zero-capacity one behaves the same way: Add
// records nothing, Recent returns nothing. Nothing above this package needs
// to know which build it is in.
const Capacity = 0

// Enabled reports whether this build records events at all.
const Enabled = false
