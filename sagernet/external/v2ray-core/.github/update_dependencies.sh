#!/usr/bin/env bash

PROJECTS=$(dirname "$0")/../..

go get -v github.com/sagernet/sing@$(git -C $PROJECTS/sing rev-parse HEAD)
go get -v github.com/sagernet/sing-shadowsocks@`git -C $PROJECTS/sing-shadowsocks rev-parse HEAD`
go mod tidy