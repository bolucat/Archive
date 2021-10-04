package libcore

import (
	"github.com/pkg/errors"
	"github.com/sagernet/gomobile/asset"
	"github.com/sirupsen/logrus"
	"github.com/xtls/xray-core/common/platform/filesystem"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"sync"
)

const (
	geoipDat       = "geoip.dat"
	geositeDat     = "geosite.dat"
	geoipVersion   = "geoip.version.txt"
	geositeVersion = "geosite.version.txt"
)

var assetsPrefix string
var internalAssetsPath string
var externalAssetsPath string

var useOfficialAssets bool
var extracted map[string]bool
var assetsAccess *sync.Mutex

type BoolFunc interface {
	Invoke() bool
}

func InitializeV2Ray(internalAssets string, externalAssets string, prefix string, useOfficial BoolFunc) error {
	assetsAccess = new(sync.Mutex)
	assetsAccess.Lock()
	extracted = make(map[string]bool)

	assetsPrefix = prefix
	internalAssetsPath = internalAssets
	externalAssetsPath = externalAssets

	filesystem.NewFileReader = func(path string) (io.ReadCloser, error) {
		_, fileName := filepath.Split(path)

		if !extracted[fileName] {
			assetsAccess.Lock()
			assetsAccess.Unlock()
		}

		paths := []string{
			internalAssetsPath + fileName,
			externalAssetsPath + fileName,
		}

		var err error

		for _, path = range paths {
			_, err = os.Stat(path)
			if err == nil {
				return os.Open(path)
			}
		}

		file, err := asset.Open(assetsPrefix + fileName)
		if err == nil {
			extracted[fileName] = true
			return file, nil
		}

		err = extractAssetName(fileName, false)
		if err != nil {
			return nil, err
		}

		for _, path = range paths {
			_, err = os.Stat(path)
			if err == nil {
				return os.Open(path)
			}
			if !os.IsNotExist(err) {
				return nil, err
			}
		}

		return nil, err
	}

	extract := func(name string) {
		err := extractAssetName(name, false)
		if err != nil {
			logrus.Warnf("Extract %s failed: %v", geoipDat, err)
		} else {
			extracted[name] = true
		}
	}

	go func() {
		defer assetsAccess.Unlock()
		useOfficialAssets = useOfficial.Invoke()

		extract(geoipDat)
		extract(geositeDat)
	}()

	return nil
}

func extractAssetName(name string, force bool) error {
	dir := externalAssetsPath
	var version string
	switch name {
	case geoipDat:
		version = geoipVersion
	case geositeDat:
		version = geositeVersion
	}

	var localVersion string
	var assetVersion string

	loadAssetVersion := func() error {
		av, err := asset.Open(assetsPrefix + version)
		if err != nil {
			return errors.WithMessage(err, "open version in assets")
		}
		b, err := ioutil.ReadAll(av)
		closeIgnore(av)
		if err != nil {
			return errors.WithMessage(err, "read internal version")
		}
		assetVersion = string(b)
		return nil
	}

	doExtract := false

	// check version

	if _, versionNotFoundError := os.Stat(dir + version); versionNotFoundError != nil {
		_, assetNotFoundError := os.Stat(dir + name)
		doExtract = assetNotFoundError != nil || force
	} else if useOfficialAssets {
		b, err := ioutil.ReadFile(dir + version)
		if err != nil {
			doExtract = true
			_ = os.RemoveAll(version)
		} else {
			localVersion = string(b)
			err = loadAssetVersion()
			if err != nil {
				return err
			}
			av, err := strconv.ParseUint(assetVersion, 10, 64)
			if err != nil {
				doExtract = assetVersion != localVersion || force
			} else {
				lv, err := strconv.ParseUint(localVersion, 10, 64)
				doExtract = err != nil || av > lv || force
			}
		}
	} else {
		doExtract = force
	}

	if doExtract {
		if assetVersion == "" {
			err := loadAssetVersion()
			if err != nil {
				return err
			}
		}
	} else {
		return nil
	}

	err := extractAsset(assetsPrefix+name+".xz", dir+name)
	if err == nil {
		err = unxz(dir + name)

	}
	if err != nil {
		return err
	}

	o, err := os.Create(dir + version)
	if err != nil {
		return err
	}
	_, err = io.WriteString(o, assetVersion)
	closeIgnore(o)
	return err
}

func extractAsset(assetPath string, path string) error {
	i, err := asset.Open(assetPath)
	if err != nil {
		return err
	}
	defer closeIgnore(i)
	o, err := os.Create(path)
	if err != nil {
		return err
	}
	defer closeIgnore(o)
	_, err = io.Copy(o, i)
	if err == nil {
		logrus.Debugf("Extract >> %s", path)
	}
	return err
}
