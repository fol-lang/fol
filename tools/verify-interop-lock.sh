#!/usr/bin/env bash
set -euo pipefail

mode="check"
case "${1-}" in
  "") ;;
  --locked) mode="locked" ;;
  *) printf 'usage: %s [--locked]\n' "$0" >&2; exit 2 ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd -P)"
lock="$root/interop.lock.toml"

fail() {
  printf 'interop lock error: %s\n' "$*" >&2
  exit 1
}

# Tier A needs only text tools, so it runs anywhere and offline. The certified
# GNU/Linux gate belongs to Tier B, which inspects real resolved checkouts.
for tool in awk grep sed sort tr xargs; do
  command -v "$tool" >/dev/null 2>&1 || fail "required tool '$tool' is unavailable"
done
test -f "$lock" || fail "missing $lock"

field() {
  local section="$1"
  local key="$2"
  awk -v wanted_section="$section" -v wanted_key="$key" '
    BEGIN { active = wanted_section == "root" }
    /^\[[^]]+\][[:space:]]*$/ {
      line = $0
      gsub(/^\[|\][[:space:]]*$/, "", line)
      active = line == wanted_section
      next
    }
    active {
      line = $0
      sub(/[[:space:]]*#.*/, "", line)
      if (line ~ "^[[:space:]]*" wanted_key "[[:space:]]*=") {
        sub(/^[^=]*=[[:space:]]*/, "", line)
        sub(/[[:space:]]*$/, "", line)
        if (line ~ /^".*"$/) {
          sub(/^"/, "", line)
          sub(/"$/, "", line)
        }
        print line
        exit
      }
    }
  ' "$lock"
}

manifest_field() {
  local manifest="$1"
  local section="$2"
  local key="$3"
  awk -v wanted_section="$section" -v wanted_key="$key" '
    /^\[[^]]+\][[:space:]]*$/ {
      line = $0
      gsub(/^\[|\][[:space:]]*$/, "", line)
      active = line == wanted_section
      next
    }
    active && $0 ~ "^[[:space:]]*" wanted_key "[[:space:]]*=" {
      line = $0
      sub(/^[^=]*=[[:space:]]*"/, "", line)
      sub(/"[[:space:]]*$/, "", line)
      print line
      exit
    }
  ' "$manifest"
}

require_equal() {
  local actual="$1"
  local expected="$2"
  local label="$3"
  test "$actual" = "$expected" || fail "$label is '$actual', expected '$expected'"
}

normalize_repository() {
  local value="$1"
  case "$value" in
    git@github.com:*) value="github.com/${value#git@github.com:}" ;;
    ssh://git@github.com/*) value="github.com/${value#ssh://git@github.com/}" ;;
    https://github.com/*) value="github.com/${value#https://github.com/}" ;;
    http://github.com/*) value="github.com/${value#http://github.com/}" ;;
  esac
  value="${value%/}"
  value="${value%.git}"
  value="${value%/}"
  printf '%s\n' "$value"
}

# ---------------------------------------------------------------------------
# Tier A — offline. The lock is the single source of truth; everything else in
# the repository must agree with it. No component checkout is consulted, so this
# tier runs on any machine with no network and no cargo cache.
# ---------------------------------------------------------------------------

require_equal "$(field root format_version)" "2" "lock format_version"
require_equal \
  "$(field root certified_target)" \
  "x86_64-unknown-linux-gnu" \
  "certified target"

for component in parc linc gerc; do
  url="$(field "$component" url)"
  repository="$(field "$component" repository)"
  package_name="$(field "$component" package)"
  crate_name="$(field "$component" crate)"
  version="$(field "$component" version)"
  revision="$(field "$component" revision)"

  test -n "$url" || fail "$component.url is empty"
  test -n "$repository" || fail "$component.repository is empty"
  test -n "$package_name" || fail "$component.package is empty"
  test -n "$crate_name" || fail "$component.crate is empty"
  test -n "$version" || fail "$component.version is empty"
  printf '%s\n' "$revision" | grep -Eq '^[0-9a-f]{40}$' \
    || fail "$component.revision is not a full lowercase commit ID"
  require_equal "$(normalize_repository "$url")" "$repository" "$component url/repository agreement"
  test -z "$(field "$component" path)" \
    || fail "$component.path must be gone: components are git dependencies now"
done

# The components pin each other. If those pins disagree, cargo resolves two
# distinct follang-parc crates and every shared contract type stops matching.
require_equal "$(field linc parc_revision)" "$(field parc revision)" "LINC's pinned PARC revision"
require_equal "$(field gerc parc_revision)" "$(field parc revision)" "GERC's pinned PARC revision"
require_equal "$(field gerc linc_revision)" "$(field linc revision)" "GERC's pinned LINC revision"

require_equal "$(field parc schema_id)" "follang.parc.source-package" "PARC schema ID"
require_equal "$(field parc schema_version)" "2" "PARC schema version"
require_equal "$(field linc schema_id)" "follang.linc.link-analysis" "LINC schema ID"
require_equal "$(field linc schema_version)" "2" "LINC schema version"
require_equal "$(field gerc schema_id)" "follang.gerc.generation" "GERC schema ID"
require_equal "$(field gerc schema_version)" "1" "GERC schema version"
require_equal "$(field linc required_feature)" "native-inspection" "LINC feature"
require_equal "$(field gerc required_feature)" "pipeline-native" "GERC feature"

# Lock vs the interop manifest.
interop_manifest="$root/lang/tooling/fol-interop/Cargo.toml"
test -f "$interop_manifest" || fail "FOL interop manifest is missing"
for component in parc linc gerc; do
  dependency_lines="$(grep -E "^${component}[[:space:]]*=" "$interop_manifest" || true)"
  require_equal \
    "$(printf '%s\n' "$dependency_lines" | grep -c . || true)" \
    "1" \
    "FOL $component dependency entry count"

  case "$dependency_lines" in
    *path*) fail "FOL $component dependency must not use a sibling path" ;;
  esac

  dependency_git="$(printf '%s\n' "$dependency_lines" \
    | sed -n 's/.*git[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p')"
  dependency_rev="$(printf '%s\n' "$dependency_lines" \
    | sed -n 's/.*rev[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p')"
  dependency_package="$(printf '%s\n' "$dependency_lines" \
    | sed -n 's/.*package[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p')"
  dependency_version="$(printf '%s\n' "$dependency_lines" \
    | sed -n 's/.*version[[:space:]]*=[[:space:]]*"=\([^"]*\)".*/\1/p')"

  require_equal "$(normalize_repository "$dependency_git")" "$(field "$component" repository)" \
    "FOL $component dependency remote"
  require_equal "$dependency_rev" "$(field "$component" revision)" "FOL $component dependency revision"
  require_equal "$dependency_package" "$(field "$component" package)" "FOL $component dependency package"
  require_equal "$dependency_version" "$(field "$component" version)" "FOL $component dependency version"

  case "$component" in
    parc)
      case "$dependency_lines" in
        *features*) fail "FOL PARC dependency must not select undeclared lock features" ;;
      esac
      ;;
    linc)
      printf '%s\n' "$dependency_lines" | grep -Fq 'default-features = false' \
        || fail "FOL interop must disable LINC default features"
      printf '%s\n' "$dependency_lines" | grep -Fq 'features = ["native-inspection"]' \
        || fail "FOL interop does not enable only LINC native-inspection"
      ;;
    gerc)
      printf '%s\n' "$dependency_lines" | grep -Fq 'default-features = false' \
        || fail "FOL interop must disable GERC default features"
      printf '%s\n' "$dependency_lines" | grep -Fq 'features = ["pipeline-native"]' \
        || fail "FOL interop does not enable only GERC pipeline-native"
      ;;
  esac
done

# Lock vs Cargo.lock: this is what makes a stale lockfile a hard failure rather
# than a silently different build.
cargo_lock="$root/Cargo.lock"
test -f "$cargo_lock" || fail "root Cargo.lock is missing"
for component in parc linc gerc; do
  package_name="$(field "$component" package)"
  revision="$(field "$component" revision)"
  resolved_source="$(awk -v wanted="$package_name" '
    $0 == "[[package]]" { in_package = 0; next }
    /^name = /  { line = $0; sub(/^name = "/, "", line); sub(/"$/, "", line); in_package = (line == wanted); next }
    in_package && /^source = / { line = $0; sub(/^source = "/, "", line); sub(/"$/, "", line); print line; exit }
  ' "$cargo_lock")"
  test -n "$resolved_source" \
    || fail "Cargo.lock has no git source for $package_name; run cargo fetch and commit the lock"
  case "$resolved_source" in
    git+*) ;;
    *) fail "$package_name is not resolved from git in Cargo.lock: $resolved_source" ;;
  esac
  printf '%s\n' "$resolved_source" | grep -Fq "rev=$revision" \
    || fail "Cargo.lock resolves $package_name at a different revision than the lock: $resolved_source"
  printf '%s\n' "$resolved_source" | grep -Fq "$(field "$component" repository)" \
    || fail "Cargo.lock resolves $package_name from a different remote: $resolved_source"
done

# A source override would silently replace a pinned component.
grep -Eq '^\[patch\."https://github\.com/fol-lang/' "$root/Cargo.toml" \
  && fail "root Cargo.toml patches a pinned interop component"
if test -f "$root/.cargo/config.toml"; then
  grep -Eq '^\[source\.|replace-with|^paths[[:space:]]*=' "$root/.cargo/config.toml" \
    && fail ".cargo/config.toml replaces a dependency source"
fi

grep -Fq "pub const CERTIFIED_INTEROP_TARGET: &str = \"$(field root certified_target)\";" \
  "$root/lang/tooling/fol-interop/src/lib.rs" \
  || fail "FOL certified target constant drifted from the lock"

# The compiled mirror takes its revisions from build.rs, which cross-checks the
# lock against Cargo.lock; assert it still does rather than hard-coding hashes.
compiled_lock="$root/lang/tooling/fol-interop/src/lock.rs"
test -f "$compiled_lock" || fail "compiled FOL interop lock mirror is missing"
for component in PARC LINC GERC; do
  grep -Fq "pub const LOCKED_${component}_REVISION: &str = env!(\"FOL_LOCKED_${component}_REVISION\");" \
    "$compiled_lock" \
    || fail "compiled $component revision must come from the build-time lock check"
  grep -Fq "LOCKED_${component}_PATH" "$compiled_lock" \
    && fail "compiled $component path must be gone: components are git dependencies now"
done

interop_book="$root/book/src/950_interop/_index.md"
for component in parc linc gerc; do
  grep -Fq "$(field "$component" revision)" "$interop_book" \
    || fail "interop book does not mirror the locked $component revision"
done
grep -Fq "$(field root pipeline_corpus_sha256)" "$interop_book" \
  || fail "interop book does not mirror the locked H5 corpus digest"

if test "$mode" != "locked"; then
  printf 'interop lock %s passed for %s (tier A)\n' "$mode" "$(field root certified_target)"
  exit 0
fi

# ---------------------------------------------------------------------------
# Tier B — needs the components resolved on disk. Only `--locked` runs this, so
# Tier A stays offline-clean.
# ---------------------------------------------------------------------------

for tool in cargo find git sha256sum uname; do
  command -v "$tool" >/dev/null 2>&1 || fail "required tool '$tool' is unavailable for --locked"
done
test "$(uname -s)" = Linux || fail "the certified interop lock gate requires GNU/Linux"

cargo fetch --locked --manifest-path "$root/Cargo.toml" >/dev/null \
  || fail "cargo fetch --locked failed; the lockfile is stale or the network is unavailable"

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
component_checkout() {
  local component="$1"
  local revision short_revision candidates
  revision="$(field "$component" revision)"
  short_revision="${revision:0:7}"
  candidates="$(find "$cargo_home/git/checkouts" -maxdepth 2 -type d \
    -path "*/${component}-*/${short_revision}" 2>/dev/null || true)"
  test "$(printf '%s\n' "$candidates" | grep -c .)" = "1" \
    || fail "expected exactly one resolved $component checkout at $short_revision"
  printf '%s\n' "$candidates"
}

for component in parc linc gerc; do
  checkout="$(component_checkout "$component")"
  require_equal "$(git -C "$checkout" rev-parse HEAD)" "$(field "$component" revision)" \
    "$component resolved checkout revision"
  manifest="$checkout/Cargo.toml"
  test -f "$manifest" || fail "$component checkout is missing Cargo.toml"
  require_equal "$(manifest_field "$manifest" package name)" "$(field "$component" package)" "$component package"
  require_equal "$(manifest_field "$manifest" package version)" "$(field "$component" version)" "$component version"
  require_equal "$(manifest_field "$manifest" lib name)" "$(field "$component" crate)" "$component crate"
done

parc_root="$(component_checkout parc)"
linc_root="$(component_checkout linc)"
gerc_root="$(component_checkout gerc)"

grep -Eq '^pub const SOURCE_PACKAGE_SCHEMA_VERSION: u32 = 2;$' \
  "$parc_root/src/contract/schema.rs" || fail "PARC source schema constant drifted"
grep -Eq '^pub const LINK_ANALYSIS_SCHEMA_VERSION: u32 = 2;$' \
  "$linc_root/src/contract/schema.rs" || fail "LINC analysis schema constant drifted"
grep -Eq '^pub const GENERATION_SCHEMA_VERSION: u16 = 1;$' \
  "$gerc_root/src/lib.rs" || fail "GERC generation schema constant drifted"
grep -Eq '^native-inspection[[:space:]]*=' "$linc_root/Cargo.toml" \
  || fail "LINC native-inspection feature is missing"
grep -Eq '^pipeline-native[[:space:]]*=' "$gerc_root/Cargo.toml" \
  || fail "GERC pipeline-native feature is missing"
grep -Fq 'version = "=0.16.0"' "$linc_root/Cargo.toml" \
  || fail "LINC does not require exact PARC 0.16.0"

corpus_digest="$({
  cd "$gerc_root"
  find tests/h5_pipeline.rs tests/pipeline-fixtures tests/pipeline_support -type f -print0 \
    | LC_ALL=C sort -z \
    | xargs -0 sha256sum \
    | sha256sum \
    | awk '{print $1}'
})"
require_equal \
  "$corpus_digest" \
  "$(field root pipeline_corpus_sha256)" \
  "GERC H5 corpus digest"

printf 'interop lock %s passed for %s (tiers A+B)\n' "$mode" "$(field root certified_target)"
