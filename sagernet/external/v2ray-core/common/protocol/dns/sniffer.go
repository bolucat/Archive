package dns

import (
	"github.com/v2fly/v2ray-core/v4/common/protocol"
	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v4/common"
	"github.com/v2fly/v2ray-core/v4/common/errors"
)

var errNotDNS = errors.New("not dns")
var errNotWanted = errors.New("not wanted")

type SniffHeader struct {
	protocol string
	domain   string
}

func (s *SniffHeader) Protocol() string {
	return s.protocol
}

func (s *SniffHeader) Domain() string {
	return s.domain
}

func SniffTCPDNS(b []byte) (*SniffHeader, error) {
	if len(b) < 2 {
		return nil, common.ErrNoClue
	}
	return SniffDNS(b[2:])
}

func SniffTCPDNSStrict(b []byte) (*SniffHeader, error) {
	if len(b) < 2 {
		return nil, common.ErrNoClue
	}
	return SniffDNSStrict(b[2:])
}

func SniffDNS(b []byte) (*SniffHeader, error) {
	var parser dnsmessage.Parser
	if common.Error2(parser.Start(b)) != nil {
		return nil, errNotDNS
	}
	question, err := parser.Question()
	if err != nil {
		return nil, errNotDNS
	}
	return &SniffHeader{"dns", question.Name.String()}, nil
}

func SniffDNSStrict(b []byte) (*SniffHeader, error) {
	var parser dnsmessage.Parser
	if common.Error2(parser.Start(b)) != nil {
		return nil, errNotDNS
	}
	question, err := parser.Question()
	if err != nil {
		return nil, errNotDNS
	}
	if question.Class != dnsmessage.ClassINET || question.Type != dnsmessage.TypeA && question.Type != dnsmessage.TypeAAAA {
		return nil, errNotWanted
	}
	domain := question.Name.String()
	if !protocol.IsValidDomain(domain) {
		return nil, errNotWanted
	}
	return &SniffHeader{"dns.strict", domain}, nil
}
