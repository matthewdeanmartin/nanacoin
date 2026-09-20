//go:build nanacoin_nodiag

package api

// DiagEnabled is false in a no-diag build: the route is not registered and
// /api/v1/diag answers 404 like any other unknown path.
const DiagEnabled = false
