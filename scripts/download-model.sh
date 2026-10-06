#!/usr/bin/env bash
# Baixa um modelo ggml do Whisper para a pasta de modelos do app.
# Uso: scripts/download-model.sh [nome-do-arquivo]
# Exemplos de nome: ggml-small.bin (padrão), ggml-small-q5_1.bin, ggml-large-v3-turbo-q5_0.bin
set -euo pipefail

MODELO="${1:-ggml-small.bin}"
IDENTIFICADOR="dev.nekro.transcritor-aulas"
PASTA="${XDG_DATA_HOME:-$HOME/.local/share}/$IDENTIFICADOR/models"

mkdir -p "$PASTA"
curl -L --fail -C - -o "$PASTA/$MODELO" \
  "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$MODELO"

echo "Modelo salvo em: $PASTA/$MODELO"
echo "Se usar outro nome além de ggml-small.bin, ajuste em Configurações > Modelo do Whisper."
