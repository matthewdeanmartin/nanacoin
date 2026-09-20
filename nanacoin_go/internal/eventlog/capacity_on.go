//go:build !nanacoin_nologs

package eventlog

// Capacity is how many events are kept. Sized to cover a page load and the
// handful of requests behind it, several times over, without being a
// meaningful share of a microcontroller's RAM.
// 58 is 90% of the previous 64 slots, rounded to the nearest whole entry.
const Capacity = 58

// Enabled reports whether this build records events at all. It is a constant
// so that TinyGo can fold the branches that read it and drop the code behind
// them, rather than shipping a runtime check.
const Enabled = true
