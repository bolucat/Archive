#!/bin/bash

source .github/env.sh

go get -v -d
go install -v github.com/sagernet/gomobile/cmd/gomobile@v0.0.0-20220616115759-d86e1a4931f0
go install -v github.com/sagernet/gomobile/cmd/gobind@v0.0.0-20220616115759-d86e1a4931f0
gomobile init
