package provider

import (
	"bytes"
	"crypto/md5"
	"os"
	"path/filepath"
	"time"

	types "github.com/Dreamacro/clash/constant/provider"
	"github.com/Dreamacro/clash/log"
)

const (
	minInterval = time.Minute * 5
)

var (
	fileMode os.FileMode = 0o666
	dirMode  os.FileMode = 0o755
)

type parser = func([]byte) (any, error)

type fetcher struct {
	name      string
	vehicle   types.Vehicle
	updatedAt time.Time
	interval  time.Duration
	done      chan struct{}
	hash      [16]byte
	parser    parser
	onUpdate  func(any)
}

func (f *fetcher) Name() string {
	return f.name
}

func (f *fetcher) VehicleType() types.VehicleType {
	return f.vehicle.Type()
}

func (f *fetcher) Initial() (any, error) {
	var (
		buf     []byte
		err     error
		isLocal bool
	)
	if stat, fErr := os.Stat(f.vehicle.Path()); fErr == nil {
		buf, err = os.ReadFile(f.vehicle.Path())
		modTime := stat.ModTime()
		f.updatedAt = modTime
		isLocal = true
	} else {
		buf, err = f.vehicle.Read()
		f.updatedAt = time.Now()
	}

	if err != nil {
		return nil, err
	}

	proxies, err := f.parser(buf)
	if err != nil {
		if !isLocal {
			return nil, err
		}

		log.Warnln("Initial local provider %s: %s", f.Name(), err.Error())

		// parse local file error, fallback to remote
		buf, err = f.vehicle.Read()
		if err != nil {
			return nil, err
		}

		proxies, err = f.parser(buf)
		if err != nil {
			return nil, err
		}

		isLocal = false
	}

	if f.vehicle.Type() != types.File && !isLocal {
		if err := safeWrite(f.vehicle.Path(), buf); err != nil {
			return nil, err
		}
	}

	f.hash = md5.Sum(buf)

	// pull proxies automatically
	if f.interval > 0 {
		go f.pullLoop()
	}

	return proxies, nil
}

func (f *fetcher) Update() (any, bool, error) {
	buf, err := f.vehicle.Read()
	if err != nil {
		return nil, false, err
	}

	now := time.Now()
	hash := md5.Sum(buf)
	if bytes.Equal(f.hash[:], hash[:]) {
		f.updatedAt = now

		os.Chtimes(f.vehicle.Path(), time.Now(), time.Now())

		return nil, true, nil
	}

	proxies, err := f.parser(buf)
	if err != nil {
		return nil, false, err
	}

	if f.vehicle.Type() != types.File {
		if err := safeWrite(f.vehicle.Path(), buf); err != nil {
			return nil, false, err
		}
	}

	f.updatedAt = now
	f.hash = hash

	return proxies, false, nil
}

func (f *fetcher) Destroy() error {
	if f.interval > 0 {
		f.done <- struct{}{}
	}
	return nil
}

func (f *fetcher) pullLoop() {
	initialInterval := f.interval - time.Since(f.updatedAt)
	if initialInterval < minInterval {
		initialInterval = minInterval
	}

	timer := time.NewTimer(initialInterval)
	defer timer.Stop()

	for {
		select {
		case <-timer.C:
			timer.Reset(f.interval)

			elm, same, err := f.Update()
			if err != nil {
				log.Warnln("[Provider] %s pull error: %s", f.Name(), err.Error())
				continue
			}

			if same {
				log.Debugln("[Provider] %s's proxies doesn't change", f.Name())
				continue
			}

			log.Infoln("[Provider] %s's proxies update", f.Name())
			if f.onUpdate != nil {
				f.onUpdate(elm)
			}
		case <-f.done:
			return
		}
	}
}

func safeWrite(path string, buf []byte) error {
	dir := filepath.Dir(path)

	if _, err := os.Stat(dir); os.IsNotExist(err) {
		if err := os.MkdirAll(dir, dirMode); err != nil {
			return err
		}
	}

	return os.WriteFile(path, buf, fileMode)
}

func newFetcher(name string, interval time.Duration, vehicle types.Vehicle, parser parser, onUpdate func(any)) *fetcher {
	return &fetcher{
		name:     name,
		interval: interval,
		vehicle:  vehicle,
		parser:   parser,
		done:     make(chan struct{}, 8),
		onUpdate: onUpdate,
	}
}
