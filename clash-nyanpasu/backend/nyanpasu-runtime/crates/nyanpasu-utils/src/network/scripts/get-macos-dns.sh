#!/bin/bash

RES=$(networksetup -getdnsservers $1)
echo $RES
