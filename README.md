# Transcritor de Aulas

App desktop que grava a aula pelo microfone do notebook, transcreve **localmente** com
whisper.cpp e pede ao Gemini uma ata estruturada (resumo, tópicos, datas, tarefas).
Tudo fica num único arquivo SQLite; se a rede cair, o áudio e a transcrição já estão
salvos e a ata pode ser gerada depois.

- Backend: Rust + Tauri v2 (captura com `cpal`, banco com `rusqlite`, `reqwest` para o Gemini)
- Frontend: HTML, CSS e JavaScript vanilla, sem bundler
- Transcrição: `whisper-cli` do whisper.cpp, empacotado como sidecar do Tauri
- Ruído: PipeWire + EasyEffects (RNNoise), veja `scripts/setup-easyeffects.md`

## Pré-requisitos (Ubuntu/Debian)

```bash
sudo apt install build-essential cmake git curl pkg-config \
  libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
# Node.js 20+ também é necessário (só para o CLI do Tauri)
```

## Primeira execução

```bash
npm install
scripts/build-whisper.sh        # compila o whisper.cpp e cria o sidecar em src-tauri/binaries/
scripts/download-model.sh       # baixa o modelo ggml-small.bin
npm run dev
```

O `tauri dev` não abre sem o sidecar: o Tauri confere que `src-tauri/binaries/whisper-cli-<triple>` existe.

Depois, em **Configurações** no app: cole a chave do Gemini (ou exporte `GEMINI_API_KEY`),
confira o modelo do Gemini e, se for usar o filtro de ruído, preencha o dispositivo e a fonte
do PipeWire.

## Fluxo

1. Escolha a disciplina e clique no botão de gravar. O `cpal` grava direto em WAV mono de
   16 kHz (reamostragem em tempo real) numa pasta oculta.
2. Ao parar, o arquivo vai para `audio/<disciplina>/` e o registro nasce com status `gravado`.
3. O pipeline roda em segundo plano, uma aula por vez:
   `transcrevendo` (whisper.cpp) → `transcrito` → `resumindo` (Gemini) → `concluido`.
4. Se algum estágio falhar o status vira `erro` e a tela oferece reprocessar. Havendo
   transcrição salva, só a ata é refeita.

## Onde ficam os dados (Linux)

| O quê | Onde |
| --- | --- |
| Banco, áudios, modelos | `~/.local/share/dev.nekro.transcritor-aulas/` (`aulas.sqlite`, `audio/`, `models/`) |
| Configurações (com a chave) | `~/.config/dev.nekro.transcritor-aulas/config.json` (permissão 600) |

## Testes

```bash
cd src-tauri && cargo test      # banco/migrations, reamostragem, validação da resposta do Gemini
```

## Estrutura

Veja `docs/arquitetura.md`. O prompt da ata está em `src-tauri/prompts/ata.md`.

## Limites conhecidos

- Transcrições acima de 600 mil caracteres são recusadas (o limite está em `gemini.rs`).
  Aulas normais ficam muito abaixo disso.
- O áudio gravado não toca dentro do app (ainda); os WAVs ficam na pasta de dados.
- Só Linux foi pensado/configurado (PipeWire, sidecar `x86_64-unknown-linux-gnu`).
