#!/usr/bin/env bash
# Android APK (Kotlin) — тот же модуль, что и в корне репозитория
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec "$REPO/scripts/build-android.sh"
