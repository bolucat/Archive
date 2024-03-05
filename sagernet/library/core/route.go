package libcore

import "github.com/sirupsen/logrus"

var networkType string

func SetNetworkType(network string) {
	if network != networkType {
		logrus.Debug("updated network type: ", network)
		networkType = network
	}
}

var wifiSSID string

func SetWifiSSID(ssid string) {
	if ssid != wifiSSID {
		logrus.Debug("updated wifi ssid: ", ssid)
		wifiSSID = ssid
	}
}
