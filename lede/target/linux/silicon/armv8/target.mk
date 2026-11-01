ARCH:=aarch64
SUBTARGET:=armv8
BOARDNAME:=Apple Silicon M1/M2 (ARM64)
CPU_TYPE:=generic

define Target/Description
  Build 64-bit Linux images for Apple Silicon using an existing
  m1n1/U-Boot EFI environment and its firmware-provided device tree.
endef
