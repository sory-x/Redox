#!/usr/bin/env bash
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
bad=0

while IFS= read -r manifest; do
    icon="$(sed -n 's/^icon=//p' "$manifest" | head -n 1)"
    if test -z "$icon" || [[ "$icon" != *.png ]]; then
        echo "ERROR: native launcher manifest has no PNG icon: $manifest -> $icon" >&2
        bad=1
    fi
done < <(find "$ROOT/recipes" -path '*/target/*/stage/usr/share/ui/apps/*' -type f | sort)

if test "$bad" -ne 0; then
    exit 1
fi

echo "PASS: all generated native launcher manifests use PNG icons"
