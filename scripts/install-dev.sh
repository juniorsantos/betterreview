#!/usr/bin/env bash
set -euo pipefail

DEST="${BETTERREVIEW_DEV_BIN:-$HOME/.local/bin}"
NAME="${1:-betterreview-dev}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
SHA="$(git rev-parse --short HEAD)"
DIRTY=""
git diff --quiet || DIRTY=" (com alterações não commitadas)"

cargo build --release --locked
mkdir -p "$DEST"
install -m 755 target/release/betterreview "$DEST/$NAME"

printf '\n%s instalado em %s\n' "$NAME" "$DEST/$NAME"
printf '  origem: %s @ %s%s\n' "$BRANCH" "$SHA" "$DIRTY"
printf '  versão: %s\n' "$("$DEST/$NAME" --version)"

case ":$PATH:" in
  *":$DEST:"*) ;;
  *) printf '\n  ATENÇÃO: %s não está no PATH. Adicione ao seu shell:\n    export PATH="%s:$PATH"\n' "$DEST" "$DEST" ;;
esac

if command -v betterreview >/dev/null 2>&1; then
  printf '\n  estável:  %s (%s)\n' "$(command -v betterreview)" "$(betterreview --version)"
fi
