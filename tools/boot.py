#!/usr/bin/env python3
"""Build a Limine ISO and optionally smoke-test it with QEMU."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "boot" / "build"
LIMINE_DIR = BUILD / "limine-12.9.6"
LIMINE_VERSION = "12.9.6"
LIMINE_ARCHIVE_URL = (
    "https://github.com/Limine-Bootloader/Limine/releases/download/"
    f"v{LIMINE_VERSION}/limine-binary.zip"
)
LIMINE_ARCHIVE_SHA256 = "d7aa69bae2cab18d2fc2ff0fdf0af8b8c8724c1be4b9460b69df774aac5c758d"
TARGET = "x86_64-unknown-none"
BOOT_MARKER = "BOOT_OK: toy-kernel booted with Limine (x86_64)"


def run(command: list[str], *, cwd: Path = ROOT) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def require_tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"Ferramenta obrigatória não encontrada no PATH: {name}")
    return path


def find_limine_file(name: str) -> Path:
    matches = [
        path
        for path in LIMINE_DIR.rglob("*")
        if path.is_file() and path.name.casefold() == name.casefold()
    ]
    if len(matches) != 1:
        raise RuntimeError(f"Arquivo {name} não encontrado em {LIMINE_DIR}")
    return matches[0]


def xorriso_path(path: Path, executable: str) -> str:
    cygpath = shutil.which("cygpath") if sys.platform == "win32" else None
    is_msys = "msys" in executable.casefold() or "cygwin" in executable.casefold()
    if cygpath is None or not is_msys:
        return str(path)
    return subprocess.run(
        [cygpath, "-u", str(path)],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def prepare_limine() -> Path:
    if not LIMINE_DIR.exists():
        LIMINE_DIR.parent.mkdir(parents=True, exist_ok=True)
        archive = BUILD / f"limine-binary-{LIMINE_VERSION}.zip"
        with urllib.request.urlopen(LIMINE_ARCHIVE_URL, timeout=30) as response:
            with archive.open("wb") as downloaded:
                shutil.copyfileobj(response, downloaded)
        digest = hashlib.sha256()
        with archive.open("rb") as downloaded:
            for block in iter(lambda: downloaded.read(1024 * 1024), b""):
                digest.update(block)
        if digest.hexdigest() != LIMINE_ARCHIVE_SHA256:
            archive.unlink(missing_ok=True)
            raise RuntimeError("SHA-256 do pacote Limine não corresponde ao valor fixado")
        with tempfile.TemporaryDirectory(prefix="limine-", dir=BUILD) as temp:
            root = Path(temp) / "package"
            root.mkdir()
            with zipfile.ZipFile(archive) as package:
                for entry in package.infolist():
                    destination = (root / entry.filename).resolve()
                    if destination != root and root not in destination.parents:
                        raise RuntimeError("Caminho inseguro no pacote do Limine")
                package.extractall(root)
            source = root / "limine-binary"
            if not source.is_dir():
                raise RuntimeError("Estrutura inesperada no pacote do Limine")
            shutil.move(str(source), str(LIMINE_DIR))
        archive.unlink()

    utility_name = "limine.exe" if sys.platform == "win32" else "limine"
    utility = find_limine_file(utility_name)
    if sys.platform != "win32":
        utility.chmod(utility.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    for filename in (
        "limine-bios.sys",
        "limine-bios-cd.bin",
        "limine-uefi-cd.bin",
        "BOOTX64.EFI",
    ):
        find_limine_file(filename)
    return utility


def run_qemu(iso: Path, firmware: str) -> None:
    qemu = Path(require_tool("qemu-system-x86_64")).resolve()
    qemu_data = qemu.parent.parent / "share" / "qemu"
    if not qemu_data.is_dir():
        raise RuntimeError(f"Diretório de dados do QEMU não encontrado: {qemu_data}")

    command = [
        str(qemu),
        "-L",
        str(qemu_data),
        "-machine",
        "q35",
        "-m",
        "256M",
        "-cdrom",
        str(iso),
        "-boot",
        "d",
        "-display",
        "none",
        "-monitor",
        "none",
        "-no-reboot",
    ]
    if firmware == "uefi":
        uefi_firmware = qemu_data / "edk2-x86_64-code.fd"
        if not uefi_firmware.is_file():
            raise RuntimeError(f"Firmware UEFI do QEMU não encontrado: {uefi_firmware}")
        command.extend(["-bios", str(uefi_firmware)])

    serial_log = BUILD / f"qemu-{firmware}-serial.log"
    serial_log.unlink(missing_ok=True)
    command.extend(["-serial", f"file:{serial_log}"])
    process = subprocess.Popen(command, cwd=ROOT)
    deadline = time.monotonic() + 30
    output = ""
    try:
        while time.monotonic() < deadline:
            if serial_log.exists():
                try:
                    output = serial_log.read_text(encoding="ascii", errors="replace")
                except PermissionError:
                    output = ""
                if BOOT_MARKER in output:
                    break
            if process.poll() is not None:
                break
            time.sleep(0.1)
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    if serial_log.exists():
        output = serial_log.read_text(encoding="ascii", errors="replace")
    if BOOT_MARKER not in output:
        raise RuntimeError(
            f"Kernel não confirmou boot BIOS/UEFI ({firmware}) em 30 s. "
            f"Saída serial:\n{output}"
        )
    print(f"QEMU confirmou o boot {firmware.upper()} pelo console serial.")


def build_iso(run_qemu_enabled: bool) -> Path:
    require_tool("cargo")
    xorriso = require_tool("xorriso")
    BUILD.mkdir(parents=True, exist_ok=True)
    limine = prepare_limine()

    run(
        [
            "cargo",
            "build",
            "--locked",
            "--package",
            "toy-kernel-boot",
            "--release",
            "--target",
            TARGET,
        ]
    )

    BUILD.mkdir(parents=True, exist_ok=True)
    final_iso = BUILD / "toy-kernel.iso"
    temporary = (
        tempfile.TemporaryDirectory(prefix="toy-kernel-")
        if sys.platform == "win32" and shutil.which("cygpath")
        else None
    )
    stage = Path(temporary.name) if temporary is not None else BUILD
    iso_root = stage / "iso_root"
    iso_root.mkdir(parents=True, exist_ok=True)
    (iso_root / "boot").mkdir(parents=True, exist_ok=True)
    (iso_root / "EFI" / "BOOT").mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "boot" / "limine.conf", iso_root / "limine.conf")
    shutil.copy2(
        ROOT / "target" / TARGET / "release" / "toy-kernel-boot",
        iso_root / "boot" / "kernel",
    )
    shutil.copy2(find_limine_file("limine-bios.sys"), iso_root / "limine-bios.sys")
    shutil.copy2(find_limine_file("limine-bios-cd.bin"), iso_root / "limine-bios-cd.bin")
    shutil.copy2(find_limine_file("limine-uefi-cd.bin"), iso_root / "limine-uefi-cd.bin")
    shutil.copy2(find_limine_file("BOOTX64.EFI"), iso_root / "EFI" / "BOOT" / "BOOTX64.EFI")

    iso = stage / "toy-kernel.iso"
    run(
        [
            xorriso,
            "-as",
            "mkisofs",
            "-R",
            "-r",
            "-J",
            "-b",
            "limine-bios-cd.bin",
            "-no-emul-boot",
            "-boot-load-size",
            "4",
            "-boot-info-table",
            "-hfsplus",
            "-apm-block-size",
            "2048",
            "--efi-boot",
            "limine-uefi-cd.bin",
            "-efi-boot-part",
            "--efi-boot-image",
            "--protective-msdos-label",
            xorriso_path(iso_root, xorriso),
            "-o",
            xorriso_path(iso, xorriso),
        ]
    )
    run([str(limine), "bios-install", str(iso)])
    if iso != final_iso:
        shutil.copy2(iso, final_iso)
        if temporary is not None:
            temporary.cleanup()
        iso = final_iso
    print(f"ISO criada: {iso}")

    if run_qemu_enabled:
        run_qemu(iso, "bios")
        run_qemu(iso, "uefi")
    return iso


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--run",
        action="store_true",
        help="iniciar a ISO no QEMU e validar os boots BIOS e UEFI",
    )
    args = parser.parse_args()
    try:
        build_iso(args.run)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"erro: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
