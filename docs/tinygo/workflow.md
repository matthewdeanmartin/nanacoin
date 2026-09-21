# The workflow

## Develop the behavior on the laptop

From `nanacoin_go/`, start the API:

```powershell
go run ./cmd/nanacoin -web ""
```

In a second terminal, start the browser application:

```powershell
cd nanacoin_ui
npm install
npm start
```

Open <http://localhost:4200>. Angular's development proxy forwards `/api` to
the Go server on port 8080. Provisioning the first household creates Nana.
The desktop executable supports a journal file; `-journal ""` selects an
in-memory run. Choose that explicitly when you want disposable state.

This loop is good for business rules, endpoint behavior and browser changes.
It does not model TinyGo's allocator, the embedded TCP stack or weak WiFi.

## Test before flashing

Run these from `nanacoin_go/`:

```powershell
go test ./internal/...
go vet ./internal/...
go test -race ./internal/...
```

The race detector needs a supported desktop toolchain; on Windows that includes
CGO and a suitable C compiler. It checks executions reached by the tests, not
every possible interleaving, and does not run on the ESP32 itself.

Useful tests exercise boundaries: a ring wrapping more than once, a failed
journal append leaving balances unchanged, simultaneous retries applying an
operation once, and escaped JSON at the maximum supported size. A test that
copies the implementation's algorithm into its expected answer is much less
likely to find a bug.

For a parser fuzzing example:

```powershell
go test ./internal/api -run '^$' -fuzz FuzzTransferParserAgainstJSON -fuzztime 10s
```

## Compile, flash, observe

Use the deploy script from `nanacoin_go/` after setting up the patched dependencies:

```powershell
.\deploy.ps1 -Port COM8 -Ssid "YourWiFi" -Password "YourPassword"
```

The script builds for `esp32s3-generic`, programs the board, and starts watching
for console output. `-NoWatch` omits the final monitor when a separate capture
process will own the console. Flashing restarts the application and loses its
RAM household. Unplugging it to move rooms also loses that state.

For a compile-only compatibility check:

```powershell
tinygo build -target=esp32s3-generic -o "$env:TEMP/nanacoin-check.elf" ./cmd/nanacoin-esp32
```

This checks the embedded build without deploying it. Use the deploy script
for an image with your network configuration. S3 firmware is flashed at
offset `0x0`; do not borrow the S2's `0x1000` command.

To watch the native console independently, from the repository root:

```powershell
python nanacoin_go/tools/boardprobe/serial_watch.py 30
```

The serial tools and their port-selection options are documented in the
[board-probe README](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/tools/boardprobe/README.md).
Avoid incidental resets when collecting evidence about a suspected crash.

## Connect the browser

Run the Angular site and enter the board's IP on its connect screen, or use
`?api=192.168.1.158` with your actual address. The board supplies the API; the
main Angular application can be hosted separately. Updating that application
does not require a firmware flash.

Cross-origin requests may require an OPTIONS preflight, especially when they
carry a bearer token. HTTPS pages calling an HTTP board can also encounter
browser mixed-content or local-network restrictions. Check the browser network
panel and board logs before diagnosing every failed connection as a CORS bug.

## Keep the development loop honest

This board is a development device, not a production service. Do not flash
an old image merely to make it responsive for a few seconds before replacing
it again. Save failure evidence, deploy the intended change, and record whether
the household was fresh or already populated when testing began.

A successful desktop test, a successful firmware build and a successful board
stress test answer three different questions. Keep all three in the workflow.
