module github.com/matthewdeanmartin/nanacoin/nanacoin_go

go 1.26.5

require (
	golang.org/x/crypto v0.57.0
	tinygo.org/x/espradio v0.3.0
)

require github.com/soypat/lneto v0.3.2

replace tinygo.org/x/espradio => ./third_party/espradio

replace github.com/soypat/lneto => ./third_party/lneto
