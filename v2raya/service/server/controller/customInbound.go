package controller

import (
	"fmt"
	"net"

	"github.com/gin-gonic/gin"
	"github.com/v2rayA/v2rayA/common"
	"github.com/v2rayA/v2rayA/db/configure"
)

func GetCustomInbound(ctx *gin.Context) {
	inbounds := configure.GetCustomInbounds()
	common.ResponseSuccess(ctx, gin.H{"inbounds": inbounds})
}

func PostCustomInbound(ctx *gin.Context) {
	var ci configure.CustomInbound
	if err := ctx.ShouldBindJSON(&ci); err != nil {
		common.ResponseError(ctx, logError("bad request"))
		return
	}
	if ci.Protocol != "socks" && ci.Protocol != "http" {
		common.ResponseError(ctx, logError(fmt.Errorf("protocol must be socks or http")))
		return
	}
	if ci.Port <= 0 || ci.Port > 65535 {
		common.ResponseError(ctx, logError(fmt.Errorf("invalid port")))
		return
	}
	if ci.Tag == "" {
		common.ResponseError(ctx, logError(fmt.Errorf("tag is required")))
		return
	}
	if net.ParseIP("0.0.0.0:"+fmt.Sprint(ci.Port)) == nil {
		// basic port check already done above
	}

	inbounds := configure.GetCustomInbounds()
	// check duplicate tag and port
	for _, existing := range inbounds {
		if existing.Tag == ci.Tag {
			common.ResponseError(ctx, logError(fmt.Errorf("tag '%s' already exists", ci.Tag)))
			return
		}
		if existing.Port == ci.Port {
			common.ResponseError(ctx, logError(fmt.Errorf("port %d is already in use by '%s'", ci.Port, existing.Tag)))
			return
		}
	}
	inbounds = append(inbounds, ci)
	if err := configure.SetCustomInbounds(inbounds); err != nil {
		common.ResponseError(ctx, logError(err))
		return
	}
	common.ResponseSuccess(ctx, gin.H{"inbounds": inbounds})
}

func DeleteCustomInbound(ctx *gin.Context) {
	var req struct {
		Tag string `json:"tag"`
	}
	if err := ctx.ShouldBindJSON(&req); err != nil || req.Tag == "" {
		common.ResponseError(ctx, logError("bad request"))
		return
	}
	inbounds := configure.GetCustomInbounds()
	newList := inbounds[:0]
	found := false
	for _, ci := range inbounds {
		if ci.Tag == req.Tag {
			found = true
			continue
		}
		newList = append(newList, ci)
	}
	if !found {
		common.ResponseError(ctx, logError(fmt.Errorf("tag '%s' not found", req.Tag)))
		return
	}
	if err := configure.SetCustomInbounds(newList); err != nil {
		common.ResponseError(ctx, logError(err))
		return
	}
	common.ResponseSuccess(ctx, gin.H{"inbounds": newList})
}
