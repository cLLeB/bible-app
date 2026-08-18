#!/usr/bin/env python3
"""Put whisper builds in bin/<backend>/, one per processor the app can run on.

The app picks between them at runtime (src-tauri/src/accel.rs). This script only
puts them where it can find them.

    python scripts/fetch_whisper_backends.py cpu          # ~8 MB
    python scripts/fetch_whisper_backends.py cpu cuda     # + ~670 MB
    python scripts/fetch_whisper_backends.py --list       # what upstream publishes

WHAT EACH BACKEND COSTS AND BUYS
--------------------------------

  cpu     ~8 MB, works everywhere, needs nothing installed. The floor, and what
          every installer has shipped so far.

  cuda    ~670 MB for the CUDA 12.4 build (~269 MB for the 11.8 one, which suits
          older drivers). NVIDIA only. Needs no CUDA toolkit on the target
          machine, only an NVIDIA display driver, because ggml talks to
          nvcuda.dll which the driver installs. This is the fastest option by a
          distance and the reason to care about GPUs at all.

  vulkan  NOT PUBLISHED as a prebuilt binary by upstream — see --list, there is
          no Vulkan asset. It has to be compiled, and the recipe is printed by
          `--how vulkan`. Do it anyway: on a 15W laptop with Intel Iris Xe it ran
          an utterance in ~1450 ms against the CPU's ~8500 ms (medians of five),
          for an identical transcript. It is the only backend covering Intel and
          AMD graphics, which is what most church laptops have. A few megabytes.

There is also a `blas` asset upstream (~21 MB, OpenBLAS on the CPU). It is
deliberately not offered here: ggml's own CPU kernels, which the app already
dispatches per-chip, generally match or beat it for whisper, so it costs 13 MB to
add a third thing to test.
"""

from __future__ import annotations

import argparse
import io
import json
import pathlib
import shutil
import sys
import urllib.request
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "bin"
API = "https://api.github.com/repos/ggml-org/whisper.cpp/releases"

# Which release asset holds which backend. Matched by substring against the asset
# name rather than pinned to an exact filename, because the CUDA assets carry the
# CUDA version in theirs and that moves between releases.
ASSETS = {
    "cpu": ["whisper-bin-x64.zip"],
    # Newest CUDA first; the 11.8 build is the fallback for older drivers.
    "cuda": ["whisper-cublas-12.4.0-bin-x64.zip", "whisper-cublas-11.8.0-bin-x64.zip"],
}

# What the app needs to find in a backend directory for it to count as installed.
REQUIRED = ["whisper-cli.exe", "whisper-server.exe"]

VULKAN_RECIPE = """Vulkan has no prebuilt release asset, so it has to be compiled once. It is worth
the trouble: measured on a 15W i5-1334U with Intel Iris Xe integrated graphics, on
eleven seconds of real speech with `small` and the app's own decode settings, five
interleaved runs each (ms of compute per utterance):

                       min    median    max
    Vulkan (Iris Xe)  1075      1452   1755
    CPU (8 threads)   5667      8487   9398

About five times faster, for a transcript identical character for character; the
GPU's worst run beat the CPU's best run threefold. It is also the only backend
covering Intel and AMD graphics, which is what most church laptops have.

Needs: git, cmake, the Vulkan SDK (https://vulkan.lunarg.com/sdk/home) and the MSVC
build tools. These exact commands produced the build measured above.

    git clone --depth 1 https://github.com/ggml-org/whisper.cpp
    cd whisper.cpp
    cmake -B build -DGGML_VULKAN=ON -DCMAKE_BUILD_TYPE=Release -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_SERVER=ON
    cmake --build build --config Release --parallel

Then copy into bin/vulkan/ of this project:

    build/bin/Release/whisper-cli.exe
    build/bin/Release/whisper-server.exe
    build/bin/Release/*.dll          (including ggml-vulkan.dll)

Target machines need no Vulkan SDK, only a current display driver: vulkan-1.dll
ships with Intel, AMD and NVIDIA drivers alike. The app checks for it before
offering the backend, and measures before preferring it.
"""


def releases(pages: int = 8, attempts: int = 3) -> list:
    """Recent releases, newest first.

    Asks for a short page rather than the default: the full list runs to several
    hundred kilobytes of chunked JSON and was reliably truncating mid-read. Retried,
    because a half-read release list should not be the reason a build fails.
    """
    url = f"{API}?per_page={pages}"
    req = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json"})
    last = None
    for _ in range(attempts):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read().decode("utf-8"))
        except Exception as e:  # network flake, truncated read, rate limit
            last = e
    raise SystemExit(f"could not read the whisper.cpp release list: {last}")


def newest_with(names: list) -> tuple:
    """The most recent release carrying one of `names`, and that asset.

    Walks back through releases rather than only checking the latest, because a
    given build is not necessarily published for every release.
    """
    for rel in releases():
        by_name = {a["name"]: a for a in rel.get("assets", [])}
        for wanted in names:
            if wanted in by_name:
                return rel["tag_name"], by_name[wanted]
    return None, None


def install(backend: str, force: bool = False) -> bool:
    target = BIN / backend
    if target.is_dir() and all((target / f).exists() for f in REQUIRED) and not force:
        print(f"  {backend}: already present ({target}) — pass --force to replace")
        return True

    if backend == "vulkan":
        print(f"\n{VULKAN_RECIPE}")
        return False

    names = ASSETS.get(backend)
    if not names:
        print(f"  {backend}: unknown backend", file=sys.stderr)
        return False

    tag, asset = newest_with(names)
    if not asset:
        print(f"  {backend}: no published asset among {names}", file=sys.stderr)
        return False

    size_mb = asset["size"] / 1e6
    print(f"  {backend}: {asset['name']} from {tag} ({size_mb:.0f} MB) ...")
    with urllib.request.urlopen(asset["browser_download_url"], timeout=1800) as r:
        blob = r.read()

    if target.is_dir():
        shutil.rmtree(target)
    target.mkdir(parents=True, exist_ok=True)
    # The archives nest everything under a folder or two. Flatten: the app expects
    # the exe and its DLLs side by side, which is also how whisper's own runtime
    # backend dispatch finds them.
    with zipfile.ZipFile(io.BytesIO(blob)) as z:
        for member in z.infolist():
            if member.is_dir():
                continue
            name = pathlib.PurePosixPath(member.filename).name
            if not name.lower().endswith((".exe", ".dll")):
                continue
            with z.open(member) as src, open(target / name, "wb") as dst:
                shutil.copyfileobj(src, dst)

    missing = [f for f in REQUIRED if not (target / f).exists()]
    if missing:
        print(f"  {backend}: WARNING — archive did not contain {missing}", file=sys.stderr)
        return False
    n = len(list(target.iterdir()))
    print(f"  {backend}: installed {n} files into {target}")
    return True


def show_list() -> None:
    rel = releases()[0]
    print(f"Latest whisper.cpp release: {rel['tag_name']}")
    for a in rel.get("assets", []):
        print(f"  {a['name']:<45} {a['size']/1e6:>7.1f} MB")
    print("\nNote there is no Vulkan asset; run --how vulkan for the build recipe.")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("backends", nargs="*", choices=["cpu", "cuda", "vulkan"], help="which to install")
    ap.add_argument("--list", action="store_true", help="show what upstream publishes")
    ap.add_argument("--how", metavar="BACKEND", help="print build instructions for a backend")
    ap.add_argument("--force", action="store_true", help="re-download even if present")
    args = ap.parse_args()

    if args.list:
        show_list()
        return
    if args.how:
        print(VULKAN_RECIPE if args.how == "vulkan" else f"No recipe for {args.how}; it is a download.")
        return
    if not args.backends:
        ap.print_help()
        return

    BIN.mkdir(parents=True, exist_ok=True)
    ok = [install(b, args.force) for b in args.backends]
    if not all(ok):
        sys.exit(1)


if __name__ == "__main__":
    main()
