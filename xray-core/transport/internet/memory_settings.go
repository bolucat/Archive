package internet

// MemoryStreamConfig is a parsed form of StreamConfig. This is used to reduce the number of Protobuf parsings.
type MemoryStreamConfig struct {
	ProtocolName     string
	ProtocolSettings interface{}
	SecurityType     string
	SecuritySettings interface{}
	SocketSettings   *SocketConfig
	DownloadSettings *MemoryStreamConfig
}

// ToMemoryStreamConfig converts a StreamConfig to MemoryStreamConfig. It returns a default non-nil MemoryStreamConfig for nil input.
func ToMemoryStreamConfig(s *StreamConfig) (*MemoryStreamConfig, error) {
	ets, err := s.GetEffectiveTransportSettings()
	if err != nil {
		return nil, err
	}

	mss := &MemoryStreamConfig{
		ProtocolName:     s.GetEffectiveProtocol(),
		ProtocolSettings: ets,
	}

	if s != nil {
		mss.SocketSettings = s.SocketSettings
	}

	if s != nil && s.HasSecuritySettings() {
		ess, err := s.GetEffectiveSecuritySettings()
		if err != nil {
			return nil, err
		}
		mss.SecurityType = s.SecurityType
		mss.SecuritySettings = ess
	}

	return mss, nil
}
