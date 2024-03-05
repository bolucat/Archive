package libcore

import (
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"sync"

	"github.com/sagernet/gomobile/asset"
	"github.com/sirupsen/logrus"
	"github.com/v2fly/v2ray-core/v5/common/platform/filesystem"
	"libcore/comm"
)

const (
	geoipDat           = "geoip.dat"
	geositeDat         = "geosite.dat"
	browserForwarder   = "index.js"
	geoipVersion       = "geoip.version.txt"
	geositeVersion     = "geosite.version.txt"
	coreVersion        = "core.version.txt"
	mozillaIncludedPem = "mozilla_included.pem"
)

var (
	assetsPrefix       string
	internalAssetsPath string
	externalAssetsPath string
)

var (
	useOfficialAssets bool
	extracted         map[string]bool
	assetsAccess      *sync.Mutex
)

type Func interface {
	Invoke() error
}

type BoolFunc interface {
	Invoke() bool
}

func InitializeV2Ray(internalAssets string, externalAssets string, prefix string, useOfficial BoolFunc, useSystemCerts BoolFunc, skipExtract bool) error {
	assetsAccess = new(sync.Mutex)
	assetsAccess.Lock()
	extracted = make(map[string]bool)

	assetsPrefix = prefix
	internalAssetsPath = internalAssets
	externalAssetsPath = externalAssets

	filesystem.NewFileSeeker = func(path string) (io.ReadSeekCloser, error) {
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

	filesystem.NewFileReader = func(path string) (io.ReadCloser, error) {
		return filesystem.NewFileSeeker(path)
	}

	if skipExtract {
		assetsAccess.Unlock()
		return nil
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
		extract(browserForwarder)

		err := extractRootCACertsPem()
		if err != nil {
			logrus.Warn(newError("failed to extract root ca certs from assets").Base(err))
			return
		}

		UpdateSystemRoots(useSystemCerts.Invoke())
	}()

	return nil
}

func extractAssetName(name string, force bool) error {
	var dir string
	if name == browserForwarder {
		dir = internalAssetsPath
	} else {
		dir = externalAssetsPath
	}
	var version string
	switch name {
	case geoipDat:
		version = geoipVersion
	case geositeDat:
		version = geositeVersion
	case browserForwarder:
		version = coreVersion
	}

	var localVersion string
	var assetVersion string

	loadAssetVersion := func() error {
		av, err := asset.Open(assetsPrefix + version)
		if err != nil {
			return newError("open version in assets").Base(err)
		}
		b, err := ioutil.ReadAll(av)
		comm.CloseIgnore(av)
		if err != nil {
			return newError("read internal version").Base(err)
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
	comm.CloseIgnore(o)
	return err
}

func extractRootCACertsPem() error {
	path := internalAssetsPath + mozillaIncludedPem
	sumPath := path + ".sha256sum"
	sumInternal, err := asset.Open(mozillaIncludedPem + ".sha256sum")
	if err != nil {
		return newError("open pem version in assets").Base(err)
	}
	defer sumInternal.Close()
	sumBytes, err := ioutil.ReadAll(sumInternal)
	if err != nil {
		return newError("read internal version").Base(err)
	}
	_, pemSha256sumNotExists := os.Stat(sumPath)
	if pemSha256sumNotExists == nil {
		sumExternal, err := ioutil.ReadFile(sumPath)
		if err == nil {
			if string(sumBytes) == string(sumExternal) {
				return nil
			}
		}
	}
	pemFile, err := os.Create(path)
	if err != nil {
		return newError("create pem file").Base(err)
	}
	defer pemFile.Close()
	pem, err := asset.Open(mozillaIncludedPem)
	if err != nil {
		return newError("open pem in assets").Base(err)
	}
	defer pem.Close()
	_, err = io.Copy(pemFile, pem)
	if err != nil {
		return newError("write pem file")
	}
	return ioutil.WriteFile(sumPath, sumBytes, 0o644)
}

func extractAsset(assetPath string, path string) error {
	i, err := asset.Open(assetPath)
	if err != nil {
		return err
	}
	defer comm.CloseIgnore(i)
	o, err := os.Create(path)
	if err != nil {
		return err
	}
	defer comm.CloseIgnore(o)
	_, err = io.Copy(o, i)
	if err == nil {
		logrus.Debugf("Extract >> %s", path)
	}
	return err
}
