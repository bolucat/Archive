package sing_tun

import (
	"errors"
	"net"
	"net/netip"

	"github.com/metacubex/mihomo/component/iface"

	"github.com/metacubex/sing-tun/control"
)

type defaultInterfaceFinder struct{}

var DefaultInterfaceFinder control.InterfaceFinder = (*defaultInterfaceFinder)(nil)

func (f *defaultInterfaceFinder) Update() error {
	iface.FlushCache()
	return nil
}

func (f *defaultInterfaceFinder) Interfaces() []control.Interface {
	ifaces, err := iface.Interfaces()
	if err != nil {
		return nil
	}
	interfaces := make([]control.Interface, 0, len(ifaces))
	for _, _interface := range ifaces {
		interfaces = append(interfaces, control.Interface(*_interface))
	}

	return interfaces
}

var errNoSuchInterface = errors.New("no such network interface")

func (f *defaultInterfaceFinder) ByName(name string) (*control.Interface, error) {
	ifaces, err := iface.Interfaces()
	if err != nil {
		return nil, err
	}
	for _, netInterface := range ifaces {
		if netInterface.Name == name {
			return (*control.Interface)(netInterface), nil
		}
	}
	_, err = net.InterfaceByName(name)
	if err == nil {
		err = f.Update()
		if err != nil {
			return nil, err
		}
		return f.ByName(name)
	}
	return nil, errNoSuchInterface
}

func (f *defaultInterfaceFinder) ByIndex(index int) (*control.Interface, error) {
	ifaces, err := iface.Interfaces()
	if err != nil {
		return nil, err
	}
	for _, netInterface := range ifaces {
		if netInterface.Index == index {
			return (*control.Interface)(netInterface), nil
		}
	}
	_, err = net.InterfaceByIndex(index)
	if err == nil {
		err = f.Update()
		if err != nil {
			return nil, err
		}
		return f.ByIndex(index)
	}
	return nil, errNoSuchInterface
}

func (f *defaultInterfaceFinder) ByAddr(addr netip.Addr) (*control.Interface, error) {
	ifaces, err := iface.Interfaces()
	if err != nil {
		return nil, err
	}
	for _, netInterface := range ifaces {
		for _, prefix := range netInterface.Addresses {
			if prefix.Contains(addr) {
				return (*control.Interface)(netInterface), nil
			}
		}
	}
	return nil, errNoSuchInterface
}
