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

for tool in awk find git grep sed sha256sum sort tr uname xargs; do
  command -v "$tool" >/dev/null 2>&1 || fail "required tool '$tool' is unavailable"
done
test "$(uname -s)" = Linux || fail "the certified interop lock gate requires GNU/Linux"
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

require_equal "$(field root format_version)" "1" "lock format_version"
require_equal \
  "$(field root certified_target)" \
  "x86_64-unknown-linux-gnu" \
  "certified target"

for component in parc linc gerc; do
  relative_path="$(field "$component" path)"
  package_name="$(field "$component" package)"
  crate_name="$(field "$component" crate)"
  version="$(field "$component" version)"
  revision="$(field "$component" revision)"
  repository="$(field "$component" repository)"

  test -n "$relative_path" || fail "$component.path is empty"
  test -n "$package_name" || fail "$component.package is empty"
  test -n "$crate_name" || fail "$component.crate is empty"
  test -n "$version" || fail "$component.version is empty"
  printf '%s\n' "$revision" | grep -Eq '^[0-9a-f]{40}$' \
    || fail "$component.revision is not a full lowercase commit ID"

  component_root="$(cd "$root/$relative_path" && pwd -P)" \
    || fail "$component path '$relative_path' is unavailable"
  manifest="$component_root/Cargo.toml"
  test -f "$manifest" || fail "$component is missing Cargo.toml"
  reported_root="$(git -C "$component_root" rev-parse --show-toplevel)"
  reported_root="$(cd "$reported_root" && pwd -P)"
  require_equal \
    "$reported_root" \
    "$component_root" \
    "$component git root"
  require_equal "$(manifest_field "$manifest" package name)" "$package_name" "$component package"
  require_equal "$(manifest_field "$manifest" package version)" "$version" "$component version"
  require_equal "$(manifest_field "$manifest" lib name)" "$crate_name" "$component crate"
  git -C "$component_root" cat-file -e "$revision^{commit}" 2>/dev/null \
    || fail "$component revision '$revision' is not present"

  if test "$mode" = "locked"; then
    require_equal "$(git -C "$component_root" rev-parse HEAD)" "$revision" "$component HEAD"
    test -z "$(git -C "$component_root" status --porcelain --untracked-files=normal --ignore-submodules=none)" \
      || fail "$component checkout is dirty"
    remote="$(normalize_repository "$(git -C "$component_root" remote get-url origin)")"
    require_equal "$remote" "$repository" "$component origin"
  fi
done

parc_root="$(cd "$root/$(field parc path)" && pwd -P)"
linc_root="$(cd "$root/$(field linc path)" && pwd -P)"
gerc_root="$(cd "$root/$(field gerc path)" && pwd -P)"

interop_manifest="$root/lang/tooling/fol-interop/Cargo.toml"
interop_manifest_root="$(dirname "$interop_manifest")"
test -f "$interop_manifest" || fail "FOL interop manifest is missing"
for component in parc linc gerc; do
  dependency_lines="$(grep -E "^${component}[[:space:]]*=" "$interop_manifest" || true)"
  require_equal \
    "$(printf '%s\n' "$dependency_lines" | grep -c . || true)" \
    "1" \
    "FOL $component dependency entry count"
  dependency_path="$(printf '%s\n' "$dependency_lines" \
    | sed -n 's/.*path[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p')"
  dependency_package="$(printf '%s\n' "$dependency_lines" \
    | sed -n 's/.*package[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p')"
  test -n "$dependency_path" || fail "FOL $component dependency path is missing"
  require_equal \
    "$dependency_package" \
    "$(field "$component" package)" \
    "FOL $component dependency package"
  dependency_root="$(cd "$interop_manifest_root/$dependency_path" && pwd -P)" \
    || fail "FOL $component dependency path '$dependency_path' is unavailable"
  locked_root="$(cd "$root/$(field "$component" path)" && pwd -P)"
  require_equal "$dependency_root" "$locked_root" "FOL $component compiled dependency root"

  case "$component" in
    parc)
      case "$dependency_lines" in
        *features*) fail "FOL PARC dependency must not select undeclared lock features" ;;
      esac
      ;;
    linc)
      printf '%s\n' "$dependency_lines" \
        | grep -Fq 'default-features = false' \
        || fail "FOL interop must disable LINC default features"
      printf '%s\n' "$dependency_lines" \
        | grep -Fq 'features = ["native-inspection"]' \
        || fail "FOL interop does not enable only LINC native-inspection"
      ;;
    gerc)
      printf '%s\n' "$dependency_lines" \
        | grep -Fq 'default-features = false' \
        || fail "FOL interop must disable GERC default features"
      printf '%s\n' "$dependency_lines" \
        | grep -Fq 'features = ["pipeline-native"]' \
        || fail "FOL interop does not enable only GERC pipeline-native"
      ;;
  esac
done

require_equal "$(field parc schema_id)" "follang.parc.source-package" "PARC schema ID"
require_equal "$(field parc schema_version)" "2" "PARC schema version"
require_equal "$(field linc schema_id)" "follang.linc.link-analysis" "LINC schema ID"
require_equal "$(field linc schema_version)" "2" "LINC schema version"
require_equal "$(field gerc schema_id)" "follang.gerc.generation" "GERC schema ID"
require_equal "$(field gerc schema_version)" "1" "GERC schema version"
require_equal "$(field linc required_feature)" "native-inspection" "LINC feature"
require_equal "$(field gerc required_feature)" "pipeline-native" "GERC feature"

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

grep -Fq "pub const CERTIFIED_INTEROP_TARGET: &str = \"$(field root certified_target)\";" \
  "$root/lang/tooling/fol-interop/src/lib.rs" \
  || fail "FOL certified target constant drifted from the lock"

compiled_lock="$root/lang/tooling/fol-interop/src/lock.rs"
test -f "$compiled_lock" || fail "compiled FOL interop lock mirror is missing"
for component in PARC LINC GERC; do
  component_key="$(printf '%s' "$component" | tr '[:upper:]' '[:lower:]')"
  relative_path="$(field "$component_key" path)"
  repository="$(field "$component_key" repository)"
  revision="$(field "$component_key" revision)"
  grep -Fq "pub const LOCKED_${component}_PATH: &str = \"$relative_path\";" \
    "$compiled_lock" \
    || fail "compiled $component path drifted from the lock"
  grep -Fq "pub const LOCKED_${component}_REPOSITORY: &str = \"$repository\";" \
    "$compiled_lock" \
    || fail "compiled $component repository drifted from the lock"
  grep -Fq "pub const LOCKED_${component}_REVISION: &str = \"$revision\";" \
    "$compiled_lock" \
    || fail "compiled $component revision drifted from the lock"
  for snapshot in \
    "$root/.github/workflows/tests.yml" \
    "$root/.github/workflows/release.yml" \
    "$root/book/src/950_interop/_index.md"; do
    grep -Fq "$revision" "$snapshot" \
      || fail "$(basename "$snapshot") does not mirror locked $component revision"
  done
done

grep -Fq "$(field root pipeline_corpus_sha256)" \
  "$root/book/src/950_interop/_index.md" \
  || fail "interop book does not mirror the locked H5 corpus digest"

printf 'interop lock %s passed for %s\n' "$mode" "$(field root certified_target)"
