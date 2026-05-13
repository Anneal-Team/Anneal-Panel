#!/usr/bin/env bash
set -euo pipefail

grep -Fq 'releases/latest' scripts/install.sh
grep -Fq 'resolve_default_release_tag' scripts/install.sh
! grep -Fq 'rolling-master' scripts/install.sh

grep -Fq 'release_name="Anneal ${version}"' .github/workflows/release-master.yml
grep -Fq 'bundle_label="${version}"' .github/workflows/release-master.yml
! grep -Fq 'refs/tags/rolling-master --force' .github/workflows/release-master.yml
grep -Fq 'paths-ignore:' .github/workflows/release-master.yml

! grep -Fq 'rolling-master' README.md
! grep -Fq 'rolling-master' README.ru.md
