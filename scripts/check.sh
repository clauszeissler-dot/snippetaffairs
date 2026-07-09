#!/usr/bin/env bash
#
# Vollständige Prüfkette — dieselbe, die die CI fährt.
# Läuft lokal vor jedem Push (siehe .githooks/pre-push) und über `bun run verify`.
#
# Bricht beim ersten Fehler ab. Kein Schritt ist optional: jeder von ihnen hat
# in diesem Repo schon mindestens einen echten Fehler gefunden.

set -euo pipefail

cd "$(dirname "$0")/.."

# Rust liegt bei Homebrew-rustup nicht zwingend im PATH einer GUI-Shell.
if [ -d /opt/homebrew/opt/rustup/bin ]; then
  export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fi

step() { printf '\n\033[36m══ %s\033[0m\n' "$1"; }
fail() { printf '\n\033[31m✗ %s\033[0m\n' "$1" >&2; exit 1; }

step "ESLint"
bun run lint || fail "ESLint meldet Probleme."

step "Frontend-Tests"
bunx vitest run || fail "Frontend-Tests rot."

step "Typecheck + Build"
bun run build || fail "tsc oder vite gescheitert."

step "Clippy (Warnungen sind Fehler)"
(cd src-tauri && cargo clippy --all-targets -- -D warnings) || fail "Clippy meldet Probleme."

step "Rust-Tests (Datenintegrität, Pfad-Schutz, IPC-Naht)"
(cd src-tauri && cargo test) || fail "Rust-Tests rot."

step "Versionen konsistent"
pkg=$(grep -m1 '"version"' package.json | sed 's/[^0-9.]//g')
conf=$(grep -m1 '"version"' src-tauri/tauri.conf.json | sed 's/[^0-9.]//g')
cargo_v=$(grep -m1 '^version' src-tauri/Cargo.toml | sed 's/[^0-9.]//g')
if [ "$pkg" != "$conf" ] || [ "$pkg" != "$cargo_v" ]; then
  fail "Versionen weichen ab: package.json=$pkg tauri.conf.json=$conf Cargo.toml=$cargo_v"
fi
echo "  alle drei auf $pkg"

printf '\n\033[32m✓ Alle Prüfungen bestanden.\033[0m\n'
