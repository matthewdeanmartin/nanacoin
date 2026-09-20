package boardmdns

import (
	"bytes"
	"encoding/binary"
	"net/netip"
	"testing"

	"github.com/soypat/lneto/dns"
	"github.com/soypat/lneto/dns/mdns"
	"github.com/soypat/lneto/ethernet"
	"github.com/soypat/lneto/ipv4"
	"github.com/soypat/lneto/x/xnet"
)

// Exercise real Ethernet/IP/UDP packets: a DNS-only test would miss multicast
// filtering, port registration, destination addressing and the IP TTL.
func TestDiscovery(t *testing.T) {
	for _, addr := range []string{"192.168.1.158", "192.168.1.159"} {
		for _, tc := range []struct {
			name string
			typ  dns.Type
		}{
			{LocalName, dns.TypeA},
			{"_http._tcp.local", dns.TypePTR},
			{"NanaCoin API._http._tcp.local", dns.TypeSRV},
			{"NanaCoin API._http._tcp.local", dns.TypeTXT},
		} {
			t.Run(addr+"/"+tc.name+"/"+tc.typ.String(), func(t *testing.T) {
				ip := netip.MustParseAddr(addr)
				server := testStack(t, ip.As4(), 1)
				if err := Register(server, ip, 80); err != nil {
					t.Fatal(err)
				}
				client := testStack(t, [4]byte{192, 168, 1, 10}, 2)
				client.SetAcceptMulticast4(true)
				mcast := mdns.IPv4MulticastAddr()
				var query mdns.Client
				if err := query.Configure(mdns.ClientConfig{LocalPort: mdns.Port, MulticastAddr: mcast[:]}); err != nil {
					t.Fatal(err)
				}
				if err := client.RegisterUDP4(&query, mcast, mdns.Port); err != nil {
					t.Fatal(err)
				}
				name, err := dns.NewName(tc.name)
				if err != nil {
					t.Fatal(err)
				}
				// Repeat to ensure responses drain and the next lookup still works.
				for range 3 {
					if err := query.StartResolve(mdns.ResolveConfig{Questions: []dns.Question{{Name: name, Type: tc.typ, Class: dns.ClassINET}}, MaxResponseAnswers: 4}); err != nil {
						t.Fatal(err)
					}
					buf := make([]byte, 1536)
					n, err := client.EgressEthernet(buf)
					if err != nil || n == 0 {
						t.Fatalf("query: %d %v", n, err)
					}
					if err := server.IngressEthernet(buf[:n]); err != nil {
						t.Fatal(err)
					}
					n, err = server.EgressEthernet(buf)
					if err != nil || n == 0 {
						t.Fatalf("response: %d %v", n, err)
					}
					eth, _ := ethernet.NewFrame(buf[:n])
					if *eth.DestinationHardwareAddr() != mdns.IPv4MulticastMAC() {
						t.Fatal("incorrect multicast MAC")
					}
					frame, _ := ipv4.NewFrame(buf[14:n])
					if frame.TTL() != 255 || *frame.DestinationAddr() != mcast {
						t.Fatal("incorrect multicast IP or TTL")
					}
					if err := client.IngressEthernet(buf[:n]); err != nil {
						t.Fatal(err)
					}
					var answers [4]dns.Resource
					count, done, err := query.AnswersCopyTo(answers[:])
					if err != nil || !done || count == 0 {
						t.Fatalf("answers: %d %v %v", count, done, err)
					}
					data := answers[0].RawData()
					switch tc.typ {
					case dns.TypeA:
						if !bytes.Equal(data, ip.AsSlice()) {
							t.Fatalf("address: %v", data)
						}
					case dns.TypeSRV:
						if len(data) < 7 || binary.BigEndian.Uint16(data[4:6]) != 80 {
							t.Fatalf("SRV: %v", data)
						}
						var target dns.Name
						if _, err := target.Decode(data, 6); err != nil || target.String() != LocalName+"." {
							t.Fatalf("SRV target: %s %v", target.String(), err)
						}
					case dns.TypePTR:
						var target dns.Name
						if _, err := target.Decode(data, 0); err != nil || target.String() != "NanaCoin API._http._tcp.local." {
							t.Fatalf("PTR: %s %v", target.String(), err)
						}
					case dns.TypeTXT:
						if !bytes.Equal(data, []byte("\x06path=/")) {
							t.Fatalf("TXT: %v", data)
						}
					}
				}
			})
		}
	}
}

func testStack(t *testing.T, ip [4]byte, id byte) *xnet.StackAsync {
	t.Helper()
	s := new(xnet.StackAsync)
	if err := s.Reset(xnet.StackConfig{Hostname: "test", RandSeed: int64(id), StaticAddress4: ip, HardwareAddress: [6]byte{2, 0, 0, 0, 0, id}, MTU: 1500, MaxActiveUDPPorts: 2}); err != nil {
		t.Fatal(err)
	}
	s.SetGatewayHardwareAddr([6]byte{2, 0, 0, 0, 0, 3})
	return s
}

func TestInvalidConfig(t *testing.T) {
	for _, addr := range []netip.Addr{{}, netip.IPv6Loopback(), netip.IPv4Unspecified()} {
		if err := Register(nil, addr, 80); err == nil {
			t.Fatal("accepted invalid address")
		}
	}
	if err := Register(nil, netip.MustParseAddr("192.168.1.158"), 0); err == nil {
		t.Fatal("accepted zero port")
	}
}
