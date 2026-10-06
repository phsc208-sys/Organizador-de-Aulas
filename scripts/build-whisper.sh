#!/usr/bin/env bash
# Compila o whisper.cpp para a CPU desta máquina e instala como sidecar do Tauri.
# Dependências (Ubuntu/Debian): sudo apt install build-essential cmake git
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
FONTE="${WHISPER_SRC:-$RAIZ/.cache/whisper.cpp}"
DESTINO="$RAIZ/src-tauri/binaries/whisper-cli-$TRIPLE"

mkdir -p "$(dirname "$FONTE")" "$RAIZ/src-tauri/binaries"

if [ -d "$FONTE/.git" ]; then
  git -C "$FONTE" pull --ff-only
else
  git clone --depth 1 https://github.com/ggml-org/whisper.cpp "$FONTE"
fi

# Libs estáticas: o binário precisa rodar sozinho, sem libwhisper.so ao lado.
cmake -S "$FONTE" -B "$FONTE/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF \
  -DWHISPER_BUILD_TESTS=OFF
cmake --build "$FONTE/build" -j"$(nproc)" --config Release --target whisper-cli

install -m 755 "$FONTE/build/bin/whisper-cli" "$DESTINO"
echo "Sidecar instalado em: $DESTINO"
