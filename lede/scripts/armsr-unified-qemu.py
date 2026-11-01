#!/usr/bin/env python3
"""Inspect an armsr armv8 EFI image and boot its default entry under QEMU.

Requires qemu-system-aarch64, mtools, and python3-pexpect. Board DTBs and
parameters are checked in the image; QEMU virt cannot emulate Qualcomm.
"""
import argparse
import hashlib
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile
import uuid
import zlib


def extract(image, offset, name, dest):
    subprocess.run(["mcopy", "-i", f"{image}@@{offset}",
                    f"::/efi/openwrt/{name}", str(dest)], check=True)


def inspect(image, output):
    with image.open("rb") as stream:
        sector = 512
        stream.seek(sector)
        header = stream.read(92)
        if header[:8] != b"EFI PART":
            sector = 4096
            stream.seek(sector)
            header = stream.read(92)
        assert header[:8] == b"EFI PART", "Missing GPT header"
        entries_lba = struct.unpack_from("<Q", header, 72)[0]
        entry_size = struct.unpack_from("<I", header, 84)[0]
        stream.seek(entries_lba * sector)
        entries = stream.read(entry_size * 2)
        assert str(uuid.UUID(bytes_le=entries[:16])) == "c12a7328-f81f-11d2-ba4b-00a0c93ec93b", "Missing EFI system partition type"
        esp_lba = struct.unpack_from("<Q", entries, 32)[0]
        root_lba = struct.unpack_from("<Q", entries, entry_size + 32)[0]
        assert root_lba > esp_lba, "Invalid ESP/rootfs layout"
        root_uuid = str(uuid.UUID(bytes_le=entries[entry_size + 16:entry_size + 32]))
        stream.seek(esp_lba * sector)
        fat_boot = stream.read(512)
        assert struct.unpack_from("<H", fat_boot, 11)[0] == sector, "FAT and disk sector sizes differ"
        assert fat_boot[82:90] == b"FAT32   ", "ESP must use FAT32"
    offset = esp_lba * sector
    extract(image, offset, "grub.cfg", output / "grub.cfg")
    config = (output / "grub.cfg").read_text()
    assert not re.search(r"@[A-Z_0-9]+@", config), "Unexpanded GRUB placeholder"
    assert 'set default="0"' in config, "Generic must be the default"
    entries = re.findall(r'menuentry\s+"([^\"]+)"\s+--id\s+(\S+)\s*\{([^}]+)\}', config)
    assert [entry[1] for entry in entries] == [
        "generic", "generic-failsafe",
        "q6a", "q6a-failsafe", "q8b", "q8b-failsafe"], "Unexpected menu order"
    kernels, roots = set(), set()
    for title, ident, commands in entries:
        linux = re.search(r"^\s*linux\s+(\S+)\s+(.+)$", commands, re.M)
        assert linux, f"Missing kernel in {ident}"
        kernels.add(linux[1])
        root = re.search(r"\broot=(\S+)", linux[2])
        assert root, f"Missing rootfs in {ident}"
        roots.add(root[1])
        assert ("failsafe=true" in linux[2]) == ident.endswith("-failsafe")
        if ident.startswith("generic"):
            assert "devicetree " not in commands
            assert "efi=noruntime" not in linux[2]
            assert "watchdog_dev.handle_boot_enabled" not in linux[2]
            assert "earlycon=qcom_geni" not in linux[2]
        else:
            board = ident.split("-")[0]
            address = "0x00994000" if board == "q6a" else "0x00884000"
            assert f"earlycon=qcom_geni,{address}" in linux[2]
            for param in ["ignore_loglevel", "loglevel=8", "keep_bootcon", "clk_ignore_unused",
                          "pd_ignore_unused", "arm64.nopauth", "efi=noruntime", "console=ttyMSM0,"]:
                assert param in linux[2], f"Missing {param} in {ident}"
            assert f"devicetree /efi/openwrt/{board}.dtb" in commands
        print(f"PASS menu: {ident}: {title}")
    assert len(kernels) == len(roots) == 1, "Menus must share one kernel and rootfs"
    root = next(iter(roots))
    if root.startswith("PARTUUID="):
        assert root[9:].lower() == root_uuid, "GRUB root UUID differs from the GPT rootfs partition"
    extract(image, offset, "bootaa64.efi", output / "bootaa64.efi")
    assert (output / "bootaa64.efi").read_bytes()[:2] == b"MZ", "Missing ARM64 EFI loader"
    kernel = next(iter(kernels)).split("/")[-1]
    extract(image, offset, kernel, output / kernel)
    assert (output / kernel).read_bytes()[:2] == b"MZ", "Shared kernel must have an EFI stub"
    for board, compat in [("q6a", b"radxa,dragon-q6a"), ("q8b", b"radxa,dragon-q8b")]:
        extract(image, offset, f"{board}.dtb", output / f"{board}.dtb")
        data = (output / f"{board}.dtb").read_bytes()
        assert data[:4] == b"\xd0\x0d\xfe\xed" and compat in data
    print("PASS shared kernel sha256:", hashlib.sha256((output / kernel).read_bytes()).hexdigest())
    print("PASS shared rootfs:", next(iter(roots)))
    return kernel


def boot(image, firmware, output, failsafe=False, menus=False):
    import pexpect
    with image.open("rb") as stream:
        stream.seek(512)
        sector = 512 if stream.read(8) == b"EFI PART" else 4096
    variables = output / ("menu-vars.fd" if menus else "failsafe-vars.fd" if failsafe else "vars.fd")
    if menus:
        # Exercise real GRUB linux/devicetree commands without handing a physical
        # board DTB to QEMU virt. The input image is never modified.
        original = (output / "grub.cfg").read_text()
        entries = re.findall(r'menuentry\s+"([^\"]+)"\s+--id\s+(\S+)\s*\{([^}]+)\}', original)
        program = original.split('set default=')[0]
        for _, ident, commands in entries:
            lines = [line.strip() for line in commands.splitlines() if line.strip()]
            for index, line in enumerate(lines):
                program += f"if {line}; then ls /GRUB_OK_{ident}_{index}; else ls /GRUB_FAIL_{ident}_{index}; fi\n"
        program += "ls /GRUB_MENUS_DONE\nreboot\n"
        test_config = output / "grub-test.cfg"
        test_config.write_text(program)
        test_image = output / "grub-test.img"
        subprocess.run(["cp", "--reflink=auto", "--sparse=always", str(image), str(test_image)], check=True)
        image = test_image
        with image.open("rb") as stream:
            stream.seek(sector + 72)
            entries_lba = struct.unpack("<Q", stream.read(8))[0]
            stream.seek(entries_lba * sector + 32)
            offset = struct.unpack("<Q", stream.read(8))[0] * sector
        subprocess.run(["mcopy", "-o", "-i", f"{image}@@{offset}", str(test_config),
                        "::/efi/openwrt/grub.cfg"], check=True)
    code = firmware / "AAVMF_CODE.fd"
    if not code.exists():
        code = firmware / "AAVMF_CODE.no-secboot.fd"
    shutil.copyfile(firmware / "AAVMF_VARS.fd", variables)
    command = ["qemu-system-aarch64", "-machine", "virt", "-cpu", "max", "-smp", "4",
               "-m", "2048", "-nographic", "-no-reboot", "-snapshot",
               "-drive", f"if=pflash,format=raw,readonly=on,file={code}",
               "-drive", f"if=pflash,format=raw,file={variables}",
               "-drive", f"if=none,id=system,format=raw,file={image}",
               "-device", f"virtio-blk-pci,drive=system,logical_block_size={sector},physical_block_size={sector}", "-netdev", "user,id=net0",
               "-device", "virtio-net-pci,netdev=net0"]
    child = pexpect.spawn(command[0], command[1:], encoding="utf-8", codec_errors="replace", timeout=240)
    log = output / ("qemu-grub-menus.log" if menus else "qemu-failsafe.log" if failsafe else "qemu-generic.log")
    try:
        with log.open("w") as stream:
            child.logfile = stream
            if menus:
                child.expect("GRUB_MENUS_DONE")
                stream.flush()
                text = log.read_text()
                assert "GRUB_FAIL_" not in text, "GRUB failed to load a menu's kernel or DTB"
                for _, ident, commands in entries:
                    for index, line in enumerate(line for line in commands.splitlines() if line.strip()):
                        assert f"GRUB_OK_{ident}_{index}" in text
                print("PASS QEMU GRUB: all six menus load their shared kernel and board DTBs")
                return
            if failsafe:
                child.expect(r"Generic")
                child.send("\x1b[B\r")
                child.expect("FAILSAFE MODE active")
            else:
                child.expect("Please press Enter to activate this console")
                child.sendline("")
            child.expect(r"root@[^\r\n]*[~/:].*#")
            child.sendline("echo ARMSR_CHECK_BEGIN; uname -r; cat /proc/cmdline; cat /etc/openwrt_release; mount | grep ' on / '; echo ARMSR_CHECK_END")
            child.expect(r"ARMSR_CHECK_BEGIN\r*\n")
            child.expect(r"ARMSR_CHECK_END\r*\n")
            result = child.before
            assert "6.18." in result and "armsr/armv8" in result
            assert " on / " in result
            stream.flush()
            assert "BPF: Invalid name" not in log.read_text(), "Module BTF validation failed"
            assert ("failsafe=true" in result) == failsafe
            print("PASS QEMU", "generic failsafe" if failsafe else "generic default")
            print(result)
    finally:
        child.terminate(force=True)
        if menus and image.exists():
            image.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("--firmware", type=Path, help="Directory containing AAVMF_CODE.fd and AAVMF_VARS.fd")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--failsafe", action="store_true")
    parser.add_argument("--check-grub-menus", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    image = args.image.resolve()
    with tempfile.TemporaryDirectory(prefix="armsr-qemu-") as temporary:
        if image.suffix == ".gz":
            raw = Path(temporary) / "system.img"
            # OpenWrt may append fwtool metadata after the gzip stream.
            decoder = zlib.decompressobj(16 + zlib.MAX_WBITS)
            with image.open("rb") as source, raw.open("wb") as target:
                while chunk := source.read(1024 * 1024):
                    target.write(decoder.decompress(chunk))
                    if decoder.eof:
                        break
            assert decoder.eof, "Truncated gzip image"
            image = raw
        inspect(image, args.output)
        if args.firmware:
            boot(image, args.firmware.resolve(), args.output, args.failsafe, args.check_grub_menus)


if __name__ == "__main__":
    main()
