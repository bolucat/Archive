package libcore

import "github.com/sirupsen/logrus"

var networkType string

func SetNetworkType(network string) {
	networkType = network
	logrus.Debug("updated network type: ", network)
}

var wifiSSID string

func SetWifiSSID(ssid string) {
	wifiSSID = ssid
	logrus.Debug("updated wifi ssid: ", ssid)
}
