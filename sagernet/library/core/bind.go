package libcore

import (
	"syscall"

	"github.com/sirupsen/logrus"
)

var upstreamNetworkName string

func bindToUpstream(fd uintptr) {
	if upstreamNetworkName == "" {
		logrus.Warn("empty upstream network name")
		return
	}
	err := syscall.BindToDevice(int(fd), upstreamNetworkName)
	if err != nil {
		logrus.Warn("failed to bind socket to upstream network ", upstreamNetworkName, ": ", err)
	}
}

func BindNetworkName(name string) {
	if name != upstreamNetworkName {
		upstreamNetworkName = name
		logrus.Debug("updated upstream network name: ", upstreamNetworkName)
	}
}
