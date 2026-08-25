#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
CRATE_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
CONSUMER_DIR="$CRATE_DIR/tests/android-consumer"
AAR="${1:?Native SDK AAR path is required}"
GRADLE="${FRESNICA_GRADLE:-gradle}"

if [ ! -s "$AAR" ]; then
  echo "Native SDK AAR not found: $AAR" >&2
  exit 1
fi
if ! command -v "$GRADLE" >/dev/null 2>&1; then
  echo "Gradle is required (set FRESNICA_GRADLE if needed)" >&2
  exit 1
fi

AAR="$(CDPATH= cd -- "$(dirname -- "$AAR")" && pwd)/$(basename -- "$AAR")"
export FRESNICA_NATIVE_AAR="$AAR"

"$GRADLE" -p "$CONSUMER_DIR" clean assembleRelease --stacktrace

OUTPUT="$(find "$CONSUMER_DIR/build/outputs/apk/release" -type f -name '*.apk' -print -quit)"
if [ -z "$OUTPUT" ] || [ ! -s "$OUTPUT" ]; then
  echo "standalone Android consumer smoke APK was not produced" >&2
  exit 1
fi

printf 'Fresnica Android raw-AAR consumer validation: OK\n'
printf '  Native SDK AAR: %s\n' "$AAR"
printf '  Consumer APK: %s\n' "$OUTPUT"
