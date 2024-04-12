package resolver

import "sync"

var blacklist struct {
	Map   map[string]struct{}
	Mutex sync.Mutex
}

func AddSystemDnsBlacklist(names ...string) {
	blacklist.Mutex.Lock()
	defer blacklist.Mutex.Unlock()
	for _, name := range names {
		blacklist.Map[name] = struct{}{}
	}
}

func RemoveSystemDnsBlacklist(names ...string) {
	blacklist.Mutex.Lock()
	defer blacklist.Mutex.Unlock()
	for _, name := range names {
		delete(blacklist.Map, name)
	}
}

func IsSystemDnsBlacklisted(names ...string) bool {
	blacklist.Mutex.Lock()
	defer blacklist.Mutex.Unlock()
	for _, name := range names {
		if _, ok := blacklist.Map[name]; ok {
			return true
		}
	}
	return false
}
