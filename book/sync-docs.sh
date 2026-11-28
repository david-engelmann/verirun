#!/usr/bin/env bash
# Stage the curated public user and design docs from `docs/` into
# `book/src/docs/` so mdBook renders them as first-class chapters, while
# rewriting links that point outside the published book to canonical GitHub
# URLs so `mdbook-linkcheck` verifies every internal link cleanly.
#
# Maintainer audit, perfection, and successor registers (`*REGISTER*.md`,
# `*ROADMAP*.md`, `AUDIT_PROGRAMME.md`, `DECISIONS.md`) stay deliberately out
# of the published book. `book/src/docs/` is git-ignored and transient.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
src_docs="$repo_root/book/src/docs"

rm -rf "$src_docs"
mkdir -p "$src_docs/media"

published_pages=(
  "TUTORIAL"
  "ARCHITECTURE"
  "INVARIANTS"
  "FAQ"
  "COMPARISON"
)

for name in "${published_pages[@]}"; do
  cp "$repo_root/docs/$name.md" "$src_docs/$name.md"
done

cp "$repo_root"/docs/media/*.svg "$src_docs/media/"
cp "$repo_root"/docs/media/*.png "$src_docs/media/"
cp "$repo_root"/docs/media/*.gif "$src_docs/media/"

maintainer_pages=(
  "AUDIT_PROGRAMME"
  "AUDIT_REGISTER"
  "PERFECTION_REGISTER"
  "PERFECTION_ROADMAP"
  "ROADMAP"
  "SUCCESSOR_REGISTER"
  "SUCCESSOR_ROADMAP"
  "README"
)

export GH="https://github.com/david-engelmann/verirun/blob/main"
export GHTREE="https://github.com/david-engelmann/verirun/tree/main"

find "$src_docs" -name '*.md' -print0 | while IFS= read -r -d '' f; do
  # 1) Links from docs/ up to repo-root files -> canonical GitHub blob URLs.
  perl -pi -e 's{\]\(\.\./(AGENTS\.md|CHANGELOG\.md|CITATION\.cff|CODE_OF_CONDUCT\.md|CONTRIBUTING\.md|Cargo\.toml|DECISIONS\.md|LICENSE-APACHE|LICENSE-MIT|Makefile|README\.md|SECURITY\.md|deny\.toml|install\.sh|pyproject\.toml|rust-toolchain\.toml)(#[^)]*)?\)}{]($ENV{GH}/$1$2)}g' "$f"

  # 2) Links from docs/ up to repo directories or source files -> GitHub blob/tree URLs.
  perl -pi -e 's{\]\(\.\./(crates|examples|fuzz|python|\.github)/([^)]*)/\)}{]($ENV{GHTREE}/$1/$2)}g' "$f"
  perl -pi -e 's{\]\(\.\./(crates|examples|fuzz|python|\.github)\)}{]($ENV{GHTREE}/$1)}g' "$f"
  perl -pi -e 's{\]\(\.\./(crates|examples|fuzz|python|\.github)/([^)]+)\)}{]($ENV{GH}/$1/$2)}g' "$f"

  # 3) Links to unpublished maintainer registers in docs/ -> GitHub blob URLs.
  for name in "${maintainer_pages[@]}"; do
    export PAGE="$name"
    perl -pi -e 's{\]\((?:\.\./docs/|\./)?\Q$ENV{PAGE}\E\.md(#[^)]*)?\)}{]($ENV{GH}/docs/$ENV{PAGE}.md$1)}g' "$f"
  done

  # 4) Links to media assets under docs/media/ -> raw GitHub URLs.
  perl -pi -e 's{\]\((?:\./)?media/([^)]+)\)}{](https://raw.githubusercontent.com/david-engelmann/verirun/main/docs/media/$1)}g' "$f"
done

echo "staged $(find "$src_docs" -name '*.md' | wc -l | tr -d ' ') docs pages into book/src/docs/"
