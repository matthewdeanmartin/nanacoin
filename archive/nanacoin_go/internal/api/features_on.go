//go:build !nanacoin_nodiag

package api

// DiagEnabled reports whether this build serves /api/v1/diag.
//
// A constant rather than a runtime setting, for the same reason the event
// log's capacity is: on the board everything is allocated at boot or it
// hits OOM later, so a switch that could be flipped at runtime would have to
// reserve its memory anyway and would save nothing. Turning diagnostics off
// means building without them.
//
// The trade is that turning them back on costs a reflash. That is the right
// way round: the flash erase budget is spent deliberately, when a board is
// actually misbehaving, rather than carrying the RAM cost on every build
// against the chance that it might.
const DiagEnabled = true
