package controller

import (
	"net/http"
	"os"
	"runtime"

	"github.com/gin-gonic/gin"
	"github.com/v2rayA/v2rayA/common"
	"github.com/v2rayA/v2rayA/conf"
	v2ray "github.com/v2rayA/v2rayA/core/v2ray"
	"github.com/v2rayA/v2rayA/core/v2ray/asset/dat"
	"github.com/v2rayA/v2rayA/core/v2ray/service"
	"github.com/v2rayA/v2rayA/core/v2ray/where"
	"github.com/v2rayA/v2rayA/db/configure"
	"github.com/v2rayA/v2rayA/pkg/util/privilege"
)

func GetVersion(ctx *gin.Context) {
	var lite int
	if conf.GetEnvironmentConfig().Lite {
		lite = 1
	}

	// Detect if running as root (Linux/macOS only)
	isRoot := false
	switch runtime.GOOS {
	case "linux", "darwin":
		isRoot = os.Geteuid() == 0
	case "windows":
		isRoot = privilege.IsRootOrAdmin()
	}

	variant, versionErr := service.CheckV5()

	// Check core version match
	coreVersionValid := true
	var coreVersionErr string
	if v2rayBinPath, err := where.GetV2rayBinPath(); err == nil {
		if err := where.CheckCoreVersion(v2rayBinPath, conf.Version); err != nil {
			coreVersionValid = false
			coreVersionErr = err.Error()
		}
	}

	common.ResponseSuccess(ctx, gin.H{
		"version":          conf.Version,
		"foundNew":         conf.FoundNew,
		"remoteVersion":    conf.RemoteVersion,
		"serviceValid":     service.IsV2rayServiceValid(),
		"v5":               versionErr == nil, // FIXME: Compomise on compatibility.
		"lite":             lite,
		"loadBalanceValid": variant == where.V2rayaCore && versionErr == nil,
		"variant":          variant,
		"os":               runtime.GOOS,
		"isRoot":           isRoot,
		"tinytunSupported": v2ray.IsTinyTunEnabled(),
		"coreVersionValid": coreVersionValid,
		"coreVersionErr":   coreVersionErr,
		"hasAccounts":      configure.HasAnyAccounts(),
	})
}

func GetRemoteGFWListVersion(ctx *gin.Context) {
	//c, err := httpClient.GetHttpClientAutomatically()
	//if err != nil {
	//	tools.ResponseError(ctx, err)
	//	return
	//}
	g, err := dat.GetRemoteGFWListUpdateTime(http.DefaultClient)
	if err != nil {
		common.ResponseError(ctx, logError(err))
		return
	}
	common.ResponseSuccess(ctx, gin.H{"remoteGFWListVersion": g.UpdateTime.Local().Format("2006-01-02")})
}
