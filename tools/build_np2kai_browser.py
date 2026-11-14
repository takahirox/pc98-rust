#!/usr/bin/env python3
"""Build a pinned, external NP2kai browser reference without changing its clone.

Requires an existing source clone, Emscripten and make. No downloads or installs.
The resulting upstream source and binaries remain outside this repository.
"""
import argparse
import shutil
import subprocess
import tarfile
from pathlib import Path

REVISION = "5939e0c6d5985c4c08fc70f289a83290e5d3e6f7"
TOOLS = Path(__file__).resolve().parent


def run(*args, cwd=None):
    subprocess.run(args, cwd=cwd, check=True)


def replace(path, old, new):
    data = path.read_bytes()
    if data.count(old) != 1:
        raise ValueError(f"Unexpected pinned source: {path.name}")
    path.write_bytes(data.replace(old, new))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.is_relative_to(TOOLS.parent) or not 1 <= args.jobs <= 32:
        parser.error("Use a new output directory outside the checkout and 1..32 jobs")
    output.mkdir()  # Never overwrite a previous reference or user directory.
    source = output / "source"
    source.mkdir()
    with subprocess.Popen(["git", "-C", str(args.source), "archive", REVISION], stdout=subprocess.PIPE) as proc:
        with tarfile.open(fileobj=proc.stdout, mode="r|") as archive:
            archive.extractall(source, filter="data")
        if proc.wait() != 0:
            raise RuntimeError("Pinned source archive failed")
    shutil.copyfile(TOOLS / "np2kai_browser_observer.c", source / "compare_observer.c")
    # Missing portable ADPCM sources and a read-only guest-clock observer.
    anchor = b"\t$(NP2_PATH)/sound/opngenc.c \\\r\n"
    replace(source / "sdl/Makefile.common", anchor, anchor +
            b"\t$(NP2_PATH)/compare_observer.c \\\r\n"
            b"\t$(NP2_PATH)/sound/ymzadpcmc.c \\\r\n"
            b"\t$(NP2_PATH)/sound/ymzadpcmg.c \\\r\n")
    # Standalone static frontends need the VFS helpers normally supplied by RetroArch.
    replace(source / "sdl/Makefile.libretro", b"ifneq ($(STATIC_LINKING), 1)\r\nSOURCES_C +=",
            b"ifeq (1, 1)\r\nSOURCES_C +=")
    params = source / "sdl/libretro/libretro_params.h"
    replace(params, b"LR_SOUNDRATE    44100.0", b"LR_SOUNDRATE    48000.0")
    replace(params, b"#define SNDSZ 782 //44100Hz/56.4fps=781.9 (sample/flame)",
            b"#define SNDSZ 851 //48000Hz/56.4fps=851.1 (samples/frame)")
    run("make", "-f", "Makefile.libretro", f"-j{args.jobs}", "platform=emscripten",
        "CC=emcc", "CXX=em++", "AR=emar", "CXX_VER=-std=c++17",
        "NP2KAI_VERSION=0.86.0.22", "NP2KAI_HASH=5939e0c6", "SUPPORT_ASYNC_CPU=0", cwd=source / "sdl")
    # The upstream .bc target is an ar archive; em++ needs its correct extension.
    shutil.copyfile(source / "sdl/np2kai_libretro_emscripten.bc", output / "np2kai.a")
    run("emcc", "-O2", "-std=c11", "-D_POSIX_C_SOURCE=200809L", "-I",
        str(source / "sdl/libretro/libretro-common/include"), "-c",
        str(TOOLS / "np2kai_browser_frontend.c"), "-o", str(output / "frontend.o"))
    run("em++", "-O2", str(output / "frontend.o"), str(output / "np2kai.a"),
        "-o", str(output / "np2.mjs"), "-sMODULARIZE=1", "-sEXPORT_ES6=1",
        "-sENVIRONMENT=web,worker", "-sALLOW_MEMORY_GROWTH=1", "-sSTACK_SIZE=8388608",
        "-sINITIAL_MEMORY=67108864", "-sFORCE_FILESYSTEM=1",
        "-sEXPORTED_RUNTIME_METHODS=FS,callMain", "-sINVOKE_RUN=0", "-sEXIT_RUNTIME=0")


if __name__ == "__main__":
    main()
