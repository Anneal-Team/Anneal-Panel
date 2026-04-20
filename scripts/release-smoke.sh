#!/usr/bin/env bash
set -euo pipefail

grep -Fq 'releases/tags/rolling-master' scripts/install.sh
grep -Fq 'resolve_default_release_tag' scripts/install.sh

grep -Fq 'release_tag="rolling-master"' .github/workflows/release-master.yml
grep -Fq 'bundle_label="rolling-master"' .github/workflows/release-master.yml
grep -Fq 'refs/tags/rolling-master --force' .github/workflows/release-master.yml
grep -Fq 'paths-ignore:' .github/workflows/release-master.yml

grep -Fq 'rolling-master' README.md
grep -Fq 'rolling-master' README.ru.md
