// Package boardmdns connects NanaCoin's HTTP service to lneto's mDNS responder.
package boardmdns

import (
	"net/netip"

	"github.com/soypat/lneto"
	"github.com/soypat/lneto/dns"
	"github.com/soypat/lneto/dns/mdns"
	"github.com/soypat/lneto/ipv4"
	"github.com/soypat/lneto/x/xnet"
)

const Hostname = "nanacoin-api"
const LocalName = Hostname + ".local"

// Register must be called once after DHCP. The stack owns the responder for
// its lifetime; the existing Ethernet pump handles it without another goroutine.
func Register(stack *xnet.StackAsync, addr netip.Addr, port uint16) error {
	if !addr.Is4() || addr.IsUnspecified() || port == 0 {
		return lneto.ErrInvalidConfig
	}
	host, err := dns.NewName(LocalName)
	if err != nil {
		return err
	}
	instance, err := dns.NewName("NanaCoin API._http._tcp.local")
	if err != nil {
		return err
	}
	multicast := mdns.IPv4MulticastAddr()
	r := new(responder)
	err = r.Configure(mdns.ClientConfig{
		LocalPort:     mdns.Port,
		MulticastAddr: multicast[:],
		Services: []mdns.Service{{
			Name: instance, Host: host, Addr: addr.AsSlice(), Port: port,
			TXTData: []byte{6, 'p', 'a', 't', 'h', '=', '/'},
		}},
	})
	if err != nil {
		return err
	}
	if err = stack.RegisterUDP4(r, multicast, mdns.Port); err != nil {
		return err
	}
	stack.SetAcceptMulticast4(true)
	return nil
}

type responder struct{ mdns.Client }

func (r *responder) Encapsulate(carrier []byte, ipOffset, frameOffset int) (int, error) {
	n, err := r.Client.Encapsulate(carrier, ipOffset, frameOffset)
	if n > 0 && ipOffset >= 0 {
		// RFC 6762 section 11: mDNS responses use IP TTL 255. The parent
		// IPv4 node calculates the checksum after this child returns.
		frame, frameErr := ipv4.NewFrame(carrier[ipOffset:])
		if frameErr != nil {
			return 0, frameErr
		}
		frame.SetTTL(255)
	}
	return n, err
}
