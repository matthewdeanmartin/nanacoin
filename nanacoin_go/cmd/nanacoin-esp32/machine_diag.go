//go:build tinygo

package main

import (
	"sync"
	"time"
	"unsafe"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/api"
)

// No extra goroutine, stack or history. Existing HTTP workers sample on the
// single active core. Only the counter is shared, protected across workers.
var machineDiagState struct {
	mu    sync.Mutex
	count uint32
}

func machineDiagnostics() api.MachineDiagnostics {
	h := readHeap()
	machineDiagState.mu.Lock()
	machineDiagState.count++
	count := machineDiagState.count
	machineDiagState.mu.Unlock()
	return api.MachineDiagnostics{
		Enabled: true, SampledAtMS: uint64(time.Since(bootTime).Milliseconds()),
		Total: h.Total, Free: h.Free, Count: count,
	}
}

func boardMachineInfo() api.MachineInfo {
	return api.MachineInfo{
		Platform: "ESP32-S3 / TinyGo", Firmware: "NanaCoin / TinyGo",
		Cores: 2, SharedBytes: unsafe.Sizeof(machineDiagState),
	}
}
