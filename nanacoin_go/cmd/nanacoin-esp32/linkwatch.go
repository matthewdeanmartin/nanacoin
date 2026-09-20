//go:build tinygo

package main

// The link watchdog: notice when the WiFi has gone and rejoin.
//
// # The gap this fills
//
// espradio raises WIFI_EVENT_STA_DISCONNECTED and clears its internal netif
// connected flag, but nothing in this program consumes that event and the
// flag is not exported. So a board that loses its association stays up,
// stays in the accept loop, and answers nothing - forever, or until someone
// power-cycles it.
//
// That failure is indistinguishable from an out-of-memory from a browser
// upstairs: in both cases the site stops loading. The black box separates
// them after the fact (an OOM reboots and leaves a note; a lost link does
// not reboot at all, so the note still says "idle" and Boots has not moved),
// but only if the board is still running to be asked. This keeps it running
// and rejoins, which is better than diagnosing it.
//
// # How it detects the loss
//
// Without an exported link-state accessor the association cannot be queried
// directly, so it is inferred from traffic. An associated board on a home
// network sees beacons and broadcast traffic constantly - ARP, mDNS, DHCP
// chatter from other devices - so espradio's RxCallbacks counter climbs even
// when nobody is talking to NanaCoin. A counter that has not moved for
// minutes means the radio is hearing nothing at all, which on a network with
// other devices on it means the association is gone.
//
// That is an inference, not a measurement, and it is wrong in one case: a
// network with genuinely no other traffic. Home networks are not that, and
// the cost of being wrong is a reassociation attempt that succeeds
// immediately and costs a second.

import (
	"time"

	"tinygo.org/x/espradio"
)

const (
	// linkCheckInterval is how often the watchdog looks. Long, because the
	// check is a heuristic and a hair trigger would reassociate on every
	// quiet moment.
	linkCheckInterval = 30 * time.Second

	// linkSilenceLimit is how long the radio may hear nothing before the
	// association is presumed lost.
	//
	// Two minutes of total radio silence on a home network means the radio is
	// not associated: beacons alone arrive every ~100 ms, and even a quiet
	// network carries ARP and mDNS. Generous, because the penalty for a false
	// positive is a reassociation and the penalty for missing a real drop is
	// a board that is up but unreachable until someone notices.
	linkSilenceLimit = 2 * time.Minute
)

// watchLink reassociates when the radio stops hearing anything.
//
// Runs forever in its own goroutine. Does not touch the HTTP server: a
// reassociation does not disturb the listener, because the lneto stack keeps
// its socket state across a link bounce and the DHCP lease is usually
// renewed to the same address.
func watchLink() {
	var (
		lastRx     uint32
		lastChange = time.Now()
		st         espradio.Stats
	)

	espradio.ReadStats(&st)
	lastRx = st.RxCallbacks

	for {
		time.Sleep(linkCheckInterval)

		espradio.ReadStats(&st)
		if st.RxCallbacks != lastRx {
			lastRx = st.RxCallbacks
			lastChange = time.Now()
			continue
		}

		silent := time.Since(lastChange)
		if silent < linkSilenceLimit {
			continue
		}

		// Nothing heard for long enough to call it. Record it in the black
		// box first: if the reassociation is what kills the board, the next
		// boot should say so rather than blaming whatever route ran last.
		note(PhaseWiFiRetry, RouteUnknown)
		println("link: no traffic for", int(silent.Seconds()), "s - reassociating")

		err := espradio.Connect(espradio.STAConfig{SSID: ssid, Password: password})
		if err != nil {
			println("link: reassociation failed:", err.Error())
			// Do not reset the timer on failure: the next pass should try
			// again rather than waiting another full silence window.
			continue
		}

		println("link: reassociated")
		lastChange = time.Now()
		espradio.ReadStats(&st)
		lastRx = st.RxCallbacks
		note(PhaseIdle, RouteUnknown)
	}
}
