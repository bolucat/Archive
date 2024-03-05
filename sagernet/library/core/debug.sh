#!/bin/bash

source .github/env.sh

BUILD="../libcore_build_debug"

rm -rf $BUILD/android \
  $BUILD/java \
  $BUILD/javac-output \
  $BUILD/src*

gomobile bind -v -cache $(realpath $BUILD) -androidapi 21 . || exit 1
rm -r libcore-sources.jar

proj=../SagerNet/app/libs
if [ -d $proj ]; then
  cp -f libcore.aar $proj
  echo ">> install $(realpath $proj)/libcore.aar"
fi
