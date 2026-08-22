#!/usr/bin/env bash
# The interop components are git dependencies pinned by revision in
# `Cargo.toml`, and `Cargo.lock` records what cargo resolved from those pins.
# That pair is the whole source of truth — there is no separate lock file to
# police. What still needs checking is that nothing quietly loosens or
# substitutes a pin, and that the book quotes the revisions actually in force.
#
# `--locked` additionally proves the pins resolve with no network and no
# manifest edits.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/lang/tooling/fol-interop/Cargo.toml"
book="$root/book/src/950_interop/_index.md"
status=0

fail() {
  printf 'interop: %s\n' "$1" >&2
  status=1
}

# The revision each component is pinned to, straight from the manifest.
revision_of() {
  sed -n "s/^$1 = .*rev = \"\([0-9a-f]\{40\}\)\".*/\1/p" "$manifest"
}

for component in parc linc gerc; do
  revision="$(revision_of "$component")"
  if [ -z "$revision" ]; then
    fail "$component is not pinned to a 40-character git revision in Cargo.toml"
    continue
  fi

  # A path dependency would silently build whatever happens to sit on disk.
  if grep -Eq "^$component = .*path = " "$manifest"; then
    fail "$component is a path dependency; interop components are pinned by revision"
  fi

  # The book states the revisions as fact, so it rots the moment one moves.
  if ! grep -Fq "$revision" "$book"; then
    fail "the interop book does not quote the pinned $component revision $revision"
  fi
done

# A source override replaces a pinned component without changing its revision.
if grep -Eq '^\[patch\."https://github\.com/fol-lang/' "$root/Cargo.toml"; then
  fail "root Cargo.toml patches a pinned interop component"
fi

if [ "${1:-}" = "--locked" ]; then
  # Proves the pins resolve exactly as committed, with no network and no edits.
  ( cd "$root" && cargo metadata --locked --format-version 1 >/dev/null ) \
    || fail "cargo cannot resolve the pinned components with --locked"
fi

if [ "$status" -eq 0 ]; then
  printf 'interop: pins verified\n'
fi
exit "$status"
