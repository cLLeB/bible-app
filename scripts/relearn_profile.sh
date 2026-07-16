#!/usr/bin/env bash
# Refresh a baked-in profile (President / Vice-President, or any name) from recordings,
# updating src-tauri/profiles.seed.json. Learns on BOTH models so both personal flavors
# get the new tuning. After this, rebuild the installers to ship it.
#
# Pass ALL the recordings you want the profile built from — the profile's entry is
# rebuilt from exactly what you pass (so include older sermons too if you want them kept).
#
# Usage (from repo root):
#   bash scripts/relearn_profile.sh "President" sermons/president/*.mp3
#   bash scripts/relearn_profile.sh "Vice-President" sermons/vice-president/*.mp3
set -e
profile="$1"; shift || true
if [ -z "$profile" ] || [ "$#" -eq 0 ]; then
  echo "usage: relearn_profile.sh <profile-name> <recording> [more recordings...]"
  exit 2
fi

exe="src-tauri/target/release/learn_cli.exe"
seed="src-tauri/profiles.seed.json"

if [ ! -f "$exe" ]; then
  echo ">> building learn_cli..."
  ( cd src-tauri && cargo build --release --bin learn_cli )
fi

echo ">> learning '$profile' on the base model ($# recording(s))..."
"$exe" base  --profile "$profile" --json "$seed" "$@"
echo ">> learning '$profile' on the small model..."
"$exe" small --profile "$profile" --json "$seed" "$@"

echo
echo ">> '$profile' updated in $seed."
echo ">> Now rebuild the installers to ship it:"
echo "     python scripts/build_flavors.py small-personal"
echo "     python scripts/build_flavors.py base-personal"
