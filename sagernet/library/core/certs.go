package libcore

import (
	"crypto/x509"
	"io/ioutil"
	_ "unsafe"

	"github.com/sirupsen/logrus"
)

//go:linkname systemRoots crypto/x509.systemRoots
var systemRoots *x509.CertPool

func updateRootCACerts() {
	x509.SystemCertPool()
	roots := x509.NewCertPool()
	pemFile, err := ioutil.ReadFile(internalAssetsPath + mozillaIncludedPem)
	if err != nil {
		logrus.Warn("failed to load root ca certificates from internal assets dir: ", err)
		return
	}
	if !roots.AppendCertsFromPEM(pemFile) {
		logrus.Warn("failed to append certificates from pem")
		return
	}
	systemRoots = roots
	logrus.Info("updated root ca certificate list")
}

//go:linkname initSystemRoots crypto/x509.initSystemRoots
func initSystemRoots()

var disableSystem bool

func UpdateSystemRoots(useSystem bool) {
	if disableSystem != useSystem {
		return
	}
	disableSystem = !disableSystem

	if useSystem {
		initSystemRoots()
		logrus.Info("reset systemRoots")
	} else {
		updateRootCACerts()
	}
}
