#!/bin/bash

source .github/env.sh

go get -v -d
go install -v github.com/sagernet/gomobile/cmd/gomobile@v0.0.0-20220214172500-89df302623c8
go install -v github.com/sagernet/gomobile/cmd/gobind@v0.0.0-20220214172500-89df302623c8
gomobile init
