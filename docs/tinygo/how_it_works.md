# How it works

## A program that boots the computer

On a laptop, the operating system loads your executable and supplies networking,
files, threads and virtual memory. On this board, flashing installs the program
that starts after reset. That image contains the application, a small runtime,
and the code needed to talk to the hardware.

TinyGo compiles Go source for small targets, including this ESP32-S3. You keep
familiar packages, types and goroutines, but cannot assume the complete desktop
Go runtime or standard library behaves identically. A package compiling on a
laptop is the first check, not the final compatibility test.

```text
Power or reset
    |
Bootloader loads firmware
    |
TinyGo runtime and application initialize
    |
Allocate bounded stores and working buffers
    |
Start radio, associate with WiFi, obtain DHCP address
    |
Accept TCP connections and serve the API
```

Network startup can take much longer than application initialization. A device
waiting for WiFi has not necessarily crashed. Its serial output is the best
way to distinguish those states.

## Three memories people confuse

| Memory | What it is for | What it does not promise |
|---|---|---|
| Flash | Firmware and, with a storage implementation, persistent data | It is not ordinary writable RAM |
| Internal RAM | Runtime, stacks, network buffers and application state | It cannot grow when traffic increases |
| PSRAM | Additional external working memory available to supported configurations | Its presence does not make it part of TinyGo's heap automatically |

The N16R8 board has 16 MB of flash and 8 MB of PSRAM. Those numbers do **not**
describe NanaCoin's available heap. The current TinyGo build operates within
a much smaller internal-memory budget. The MicroPython build's multi-megabyte
heap on this same board is a different runtime configuration.

A heap is memory managed for objects whose lifetime outlasts a function call.
A stack holds function-call state and local working data. Both consume RAM.
Putting a large array on the stack can avoid a heap allocation and still crash
the program by exhausting the stack.

## What transfers from desktop Go

| Familiar idea | On the board |
|---|---|
| Packages and interfaces | Useful for keeping application logic independent of hardware |
| Unit tests | Run quickly on the laptop; hardware behavior still needs a board test |
| Garbage collection | Available, but collection cannot manufacture memory or guarantee a large contiguous free region |
| Goroutines | Useful for overlapping work, but not evidence of execution on multiple CPUs |
| Files | Require a concrete backend; there is no general-purpose disk supplied by this application |
| HTTP handlers | Shared with desktop code through an embedded transport adapter |

NanaCoin therefore has two entry points. The desktop entry point uses the
ordinary Go HTTP server and can open a journal file. The embedded entry point
brings up the radio and a bounded HTTP server. Both call the same domain code.

## Words used in the rest of the guide

- **Firmware:** the compiled image flashed onto the board.
- **Target:** TinyGo's description of the chip, runtime and build settings.
- **Bootloader:** the small program responsible for starting or loading firmware.
- **Serial console:** text emitted over USB, independent of a working web server.
- **DHCP:** the router assigning the board an IP address.
- **OOM:** out of memory; an allocation cannot be satisfied.
- **Bounded:** a resource has an explicit maximum and defined behavior at that maximum.

The last word is the design principle that appears everywhere in NanaCoin.
Bounded does not mean impossible to exhaust. It means exhaustion has a planned
outcome rather than silently turning into unlimited growth.
