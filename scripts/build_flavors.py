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

--reuse-data: skip the wipe+re-download of data/*.canonical.json; use whatever is
  already in data/. Useful when translations were downloaded in a prior run that
  was interrupted before the Tauri compile, or when bolls.life is unreachable.
  The flavor's tier + model env-vars are still baked into the binary correctly.
"""
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
MODELS = ["base", "small", "tiny"]

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
    whole bin/ dir goes along."""
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
        for f in BIN_DIR.iterdir():
            if f.is_file():
                shutil.copy2(f, BUNDLED / "bin" / f.name)
    else:
        print("  WARNING: no bin/ dir — whisper binary won't be bundled (STT unavailable)", file=sys.stderr)


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
    try:
        subprocess.check_call(["npm", "run", "tauri", "build"], cwd=ROOT, env=env, shell=(os.name == "nt"))
    finally:
        if hidden and hidden.exists():
            hidden.rename(personal_songs)
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
    argv = [a for a in argv if a != "--reuse-data"]
    targets = list(fl) if argv[0] == "--all" else argv
    for t in targets:
        if t not in fl:
            print(f"unknown flavor '{t}'. Options: {', '.join(fl)}", file=sys.stderr)
            continue
        build(t, fl[t], reuse_data=reuse_data)


if __name__ == "__main__":
    main(sys.argv[1:])
