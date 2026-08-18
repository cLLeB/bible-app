#!/usr/bin/env python3
"""Build the app's release flavors.

A FLAVOR = a whisper model + a license tier:
  * tier 'distribution' — public-domain translations only (safe to share)
  * tier 'personal'     — all translations incl. copyrighted, for the builder's
                          OWN private use (never distributed)

Six shippable flavors (model x tier):
    base-distribution   base-personal
    small-distribution  small-personal
    medium-distribution medium-personal
Plus 'testing' — all models + personal tier — for local QA of everything at once.

For each flavor this script:
  1. populates data/ with the flavor's translations (imported from bolls.life;
     public-domain always, copyrighted only for personal/testing),
  2. bakes the tier + model set into the binary via env vars, and
  3. runs `npm run tauri build` (installer lands in
     src-tauri/target/release/bundle/).

Requires whisper model .bin files in models/. Nothing here runs at app runtime —
the app stays offline.

Translation data is sourced in this priority order (no unnecessary downloads):
  1. data/ — whatever is already there (--reuse-data keeps it untouched)
  2. src-tauri/target/release/data/ and _up_/data/ — Tauri's own resource
     staging dirs, populated by previous builds on this machine
  3. bolls.life / getbible.net — only fetched for files absent from both above

Usage:
  python scripts/build_flavors.py testing
  python scripts/build_flavors.py base-distribution
  python scripts/build_flavors.py --all
  python scripts/build_flavors.py --all --reuse-data   # skip translation download if data/ already populated
  python scripts/build_flavors.py small-personal --with-gpu    # + graphics cards

Graphics acceleration. Nobody installing the app does anything for any of this: it
detects what the machine has, proves the backend can actually transcribe before
trusting it, and falls back to the processor otherwise.

  --with-gpu     everything: Vulkan (Intel, AMD and NVIDIA alike) and CUDA (NVIDIA,
                 faster still). Vulkan is compiled here, which needs the Vulkan SDK
                 and takes 10-20 minutes the first time; CUDA is a ~670 MB download.
  --with-vulkan  Vulkan only. A few megabytes in the installer, covers every GPU
                 vendor, and on a 15W Iris Xe laptop it was about five times faster
                 than the processor. The best value of the three.
  --with-cuda    CUDA only. NVIDIA machines, where it beats Vulkan.

Whatever is missing is simply not bundled; the build still succeeds and those
machines use the processor.

--reuse-data: skip the wipe+re-download of data/*.canonical.json; use whatever is
  already in data/. Useful when translations were downloaded in a prior run that
  was interrupted before the Tauri compile, or when bolls.life is unreachable.
  The flavor's tier + model env-vars are still baked into the binary correctly.
"""
import json
import os
import shutil
import subprocess
import sys
import pathlib

# bolls codes. Keep in sync with src-tauri/src/translations.rs.
PUBLIC_DOMAIN = ["BSB", "WEB", "KJV", "ASV", "YLT", "DARBY", "BBE", "GNV", "DRB", "WBT", "LXXE", "LSV"]
LICENSED = ["NIV", "NLT", "ESV", "NKJV", "NASB", "CSB17", "AMP", "MSG", "NET",
            "GNT", "GNTD", "RSV", "NRSVCE", "CEB", "CEVD", "CJB", "TLV", "LSB",
            "MEV", "ISV", "ERV", "NLV", "NABRE"]
# medium was left out while everything ran on the processor, where it could not keep
# up with live speech. A graphics card changes that (see src-tauri/src/accel.rs), so
# it is a flavor worth building again. Its model file is ~1.5 GB, which the installer
# carries, so it is not a default anyone should reach for without meaning to.
MODELS = ["base", "small", "tiny", "medium"]

ROOT = pathlib.Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
MODELS_DIR = ROOT / "models"
# Per-flavor installers collected here. NOT `dist/` — that is Vite's frontendDist,
# which `vite build` empties at the start of every `tauri build`, wiping collections.
DIST = ROOT / "installers"
BIN_DIR = ROOT / "bin"        # whisper-cli.exe + its CPU backend DLLs
# Staging dir that tauri.conf `resources` bundles: filled per-flavor with just
# that flavor's whisper model(s) + the whisper binary, so shipped installers
# carry STT and run fully offline on any machine.
BUNDLED = ROOT / "bundled"
# Local translation cache: Tauri's own resource staging dirs from prior builds.
# Priority: data/ → CACHE_DIRS → network download.
CACHE_DIRS = [
    ROOT / "src-tauri" / "target" / "release" / "data",
    ROOT / "src-tauri" / "target" / "release" / "_up_" / "data",
]


def flavors() -> dict:
    out = {}
    for m in MODELS:
        out[f"{m}-distribution"] = {"tier": "distribution", "models": [m], "codes": PUBLIC_DOMAIN}
        out[f"{m}-personal"] = {"tier": "personal", "models": [m], "codes": PUBLIC_DOMAIN + LICENSED}
    out["testing"] = {"tier": "personal", "models": ["base", "small", "medium"], "codes": PUBLIC_DOMAIN + LICENSED}
    return out


def prepare_translations(codes: list, personal: bool, reuse_data: bool = False) -> None:
    """Rebuild data/*.canonical.json to exactly this flavor's translation set."""
    if not reuse_data:
        for f in DATA.glob("*.canonical.json"):
            f.unlink()

    # Always try to restore from local Tauri build cache before downloading
    for code in codes:
        target = DATA / f"{code.lower()}.canonical.json"
        if not target.exists():
            for cache_dir in CACHE_DIRS:
                cached = cache_dir / f"{code.lower()}.canonical.json"
                if cached.exists():
                    shutil.copy2(cached, target)
                    print(f"  restored {code} from local cache ({cache_dir.name})")
                    break

    # Download only what is still missing after the cache restore
    missing = [c for c in codes if not (DATA / f"{c.lower()}.canonical.json").exists()]
    if missing:
        print(f"  downloading missing translation(s): {missing}")
        importer = str(ROOT / "scripts" / "import_bolls.py")
        pd_miss = [c for c in missing if c in PUBLIC_DOMAIN]
        lic_miss = [c for c in missing if c not in PUBLIC_DOMAIN]
        if pd_miss:
            subprocess.check_call([sys.executable, importer, *pd_miss], cwd=ROOT)
        if lic_miss and personal:
            subprocess.check_call([sys.executable, importer, *lic_miss, "--force"], cwd=ROOT)


def check_models(models: list) -> None:
    missing = [m for m in models if not (MODELS_DIR / f"ggml-{m}.en.bin").exists()]
    if missing:
        print(
            f"WARNING: missing whisper models {missing} — place ggml-<model>.en.bin in {MODELS_DIR}",
            file=sys.stderr,
        )


def stage_runtime(models: list) -> None:
    """Fill bundled/ with THIS flavor's whisper model(s), so tauri.conf `resources`
    bundle exactly what the flavor needs (and no other model). Cleared each build
    so flavors never leak assets into each other.

    The whisper binary ships with per-CPU backend DLLs and picks an optimized one
    at runtime; that dispatch is worth ~6x over a statically linked build, so the
    whole bin/ dir goes along.

    bin/ may also hold per-processor subdirectories (bin/cpu, bin/cuda, bin/vulkan
    — see scripts/fetch_whisper_backends.py). They are copied through as they are,
    and the app picks between them at runtime. A flat bin/ with no subdirectories
    is still a valid CPU-only build, which is what every installer so far has
    been."""
    if BUNDLED.exists():
        shutil.rmtree(BUNDLED)
    (BUNDLED / "models").mkdir(parents=True, exist_ok=True)
    (BUNDLED / "bin").mkdir(parents=True, exist_ok=True)
    for m in models:
        src = MODELS_DIR / f"ggml-{m}.en.bin"
        if src.exists():
            shutil.copy2(src, BUNDLED / "models" / src.name)
        else:
            print(f"  WARNING: model {src.name} missing — STT unavailable in this flavor", file=sys.stderr)
    if BIN_DIR.is_dir():
        backends = []
        for f in BIN_DIR.iterdir():
            if f.is_file():
                shutil.copy2(f, BUNDLED / "bin" / f.name)
            elif f.is_dir():
                shutil.copytree(f, BUNDLED / "bin" / f.name, dirs_exist_ok=True)
                backends.append(f.name)
        if backends:
            print(f"  processors bundled: {', '.join(sorted(backends))}")
        else:
            print("  processors bundled: cpu only (see scripts/fetch_whisper_backends.py)")
    else:
        print("  WARNING: no bin/ dir — whisper binary won't be bundled (STT unavailable)", file=sys.stderr)


# makensis is a 32-bit program and maps its payload into memory, so it fails once a
# flavor gets big enough:
#     Internal compiler error #12345: error mmapping file (...) is out of range
# medium (1.5 GB of model) plus CUDA (574 MB) is comfortably past it. WiX has no such
# trouble, so those flavors ship as an .msi and skip the .exe rather than failing.
NSIS_PAYLOAD_LIMIT = 1_800_000_000


def payload_bytes() -> int:
    total = 0
    for f in BUNDLED.rglob("*"):
        if f.is_file():
            total += f.stat().st_size
    for f in DATA.glob("*.canonical.json"):
        total += f.stat().st_size
    return total


def backend_resources_config():
    """A --config override listing each per-processor directory explicitly.

    Tauri's resource map does not preserve directory structure: a `bin/**/*` glob
    copies every match into the single destination, so bin/ggml-base.dll and
    bin/cpu/ggml-base.dll both land on bin/ggml-base.dll. WiX then refuses to build,
    which is at least loud — but the underlying problem was that only one of them
    would ever have arrived.

    One entry per directory keeps them apart. They cannot be written statically in
    tauri.conf.json because which backends exist varies by build, and a glob matching
    nothing is not worth risking, so the map is built from what was actually staged.

    Returns the path to a temporary config file, or None when there is nothing but
    the loose CPU files (in which case tauri.conf.json already covers it).
    """
    staged = BUNDLED / "bin"
    dirs = sorted(d.name for d in staged.iterdir() if d.is_dir()) if staged.is_dir() else []
    if not dirs:
        return None
    resources = {
        "../data/*.canonical.json": "data/",
        "../bundled/models/*.bin": "models/",
        "../bundled/bin/*": "bin/",
    }
    for d in dirs:
        resources[f"../bundled/bin/{d}/*"] = f"bin/{d}/"
    path = ROOT / "src-tauri" / ".backends.tauri.conf.json"
    path.write_text(json.dumps({"bundle": {"resources": resources}}, indent=2), encoding="utf-8")
    print(f"  packaging processors: {', '.join(dirs)}")
    return path


def verify_packaged(models: list) -> None:
    """Check that what we staged actually reached the package.

    Worth its own step because of how this failed once: tauri.conf listed
    `bundled/bin/*`, a single-level glob, so the per-processor subdirectories were
    copied into bundled/ by stage_runtime, reported as bundled, and then silently
    left out of the installer. The build succeeded, the log said "processors
    bundled: cpu, cuda, vulkan", and the installer had none of them.

    Nothing downstream would have caught it either: the app falls back to the
    processor exactly as designed when a backend is absent, so the only symptom
    would have been a church wondering why it was slow.
    """
    staged = BUNDLED / "bin"
    packaged = ROOT / "src-tauri" / "target" / "release" / "bin"
    if not staged.is_dir() or not packaged.is_dir():
        return

    want = sorted(d.name for d in staged.iterdir() if d.is_dir())
    got = sorted(d.name for d in packaged.iterdir() if d.is_dir())
    missing = [b for b in want if b not in got]
    if missing:
        print(f"  ERROR: staged {want} but the package only has {got or 'none'}; "
              f"missing {missing}", file=sys.stderr)
        print("  The installer would run on the processor only. Check the "
              "`resources` globs in src-tauri/tauri.conf.json.", file=sys.stderr)
        sys.exit(1)

    for m in models:
        if not (packaged.parent / "models" / f"ggml-{m}.en.bin").exists():
            print(f"  WARNING: model {m} was staged but is not in the package",
                  file=sys.stderr)
    if want:
        print(f"  verified in package: {', '.join(want)}")


def build(name: str, spec: dict, reuse_data: bool = False) -> None:
    print(f"\n=== Building flavor '{name}' (tier={spec['tier']}, models={spec['models']}) ===")
    check_models(spec["models"])
    prepare_translations(spec["codes"], spec["tier"] == "personal", reuse_data=reuse_data)
    stage_runtime(spec["models"])
    # Never bundle personal songs into a distribution build.
    personal_songs = DATA / "personal.songs.json"
    hidden = None
    if spec["tier"] == "distribution" and personal_songs.exists():
        hidden = personal_songs.with_name("personal.songs.hidden")
        personal_songs.rename(hidden)
    env = dict(os.environ)
    env["BIBLE_APP_TIER"] = spec["tier"]
    env["BIBLE_APP_MODELS"] = ",".join(spec["models"])
    # Wipe the shared bundle dir first. It is not per-flavor, so anything left in it
    # from the previous build gets picked up by collect_outputs and filed under this
    # flavor's name. That is how a medium-personal-...-setup.exe appeared that was
    # actually the small build: same shared directory, stale file, plausible name.
    bundle_dir = ROOT / "src-tauri" / "target" / "release" / "bundle"
    if bundle_dir.exists():
        shutil.rmtree(bundle_dir, ignore_errors=True)

    cmd = ["npm", "run", "tauri", "build"]
    extra = []
    override = backend_resources_config()
    if override:
        extra += ["--config", str(override)]
    size = payload_bytes()
    if size > NSIS_PAYLOAD_LIMIT:
        extra += ["--bundles", "msi"]
        print(f"  payload is {size/1e9:.1f} GB, past what makensis can map: "
              f"building the .msi only")
    if extra:
        cmd += ["--", *extra]
    try:
        subprocess.check_call(cmd, cwd=ROOT, env=env, shell=(os.name == "nt"))
    finally:
        if hidden and hidden.exists():
            hidden.rename(personal_songs)
        if override:
            override.unlink(missing_ok=True)
    verify_packaged(spec["models"])
    collect_outputs(name)
    print(f"=== '{name}' built. Installers in {DIST / name} ===")


def collect_outputs(name: str) -> None:
    """Copy this flavor's installers out of the shared bundle/ dir into
    dist/<name>/ (prefixed with the flavor), since the next build overwrites
    bundle/ with an identically-named installer."""
    bundle = ROOT / "src-tauri" / "target" / "release" / "bundle"
    out = DIST / name
    out.mkdir(parents=True, exist_ok=True)
    collected = []
    for sub, pattern in (("msi", "*.msi"), ("nsis", "*.exe")):
        for f in sorted((bundle / sub).glob(pattern)):
            dest = out / f"{name}-{f.name}"
            shutil.copy2(f, dest)
            collected.append(dest.name)
    if collected:
        print(f"  collected {len(collected)} installer(s): {', '.join(collected)}")
    else:
        print(f"  WARNING: no installers found under {bundle} to collect", file=sys.stderr)


def main(argv: list) -> None:
    fl = flavors()
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        print("Flavors:", ", ".join(fl))
        return
    reuse_data = "--reuse-data" in argv
    # Opting the build in to graphics-card acceleration. Doing it here means nobody
    # installing the app has to do anything at all: whatever is in bin/ is bundled,
    # and the app detects, proves and uses it by itself.
    want = []
    if "--with-gpu" in argv:
        want = ["cpu", "vulkan", "cuda"]
    elif "--with-vulkan" in argv:
        want = ["cpu", "vulkan"]
    elif "--with-cuda" in argv:
        want = ["cpu", "cuda"]
    argv = [a for a in argv
            if a not in ("--reuse-data", "--with-gpu", "--with-vulkan", "--with-cuda")]
    if want:
        print(f"=== Preparing whisper builds: {', '.join(want)} ===")
        # Not check_call: a missing Vulkan SDK or an unreachable CUDA download should
        # not throw away a build that is otherwise fine. The app falls back to the
        # processor for whatever is absent, which is exactly what it is designed to do.
        rc = subprocess.call(
            [sys.executable, str(ROOT / "scripts" / "fetch_whisper_backends.py"), *want],
            cwd=ROOT,
        )
        if rc != 0:
            print("  WARNING: not every backend could be prepared; building with what "
                  "is in bin/", file=sys.stderr)
    targets = list(fl) if argv[0] == "--all" else argv
    for t in targets:
        if t not in fl:
            print(f"unknown flavor '{t}'. Options: {', '.join(fl)}", file=sys.stderr)
            continue
        build(t, fl[t], reuse_data=reuse_data)


if __name__ == "__main__":
    main(sys.argv[1:])
