# Arquitetura

```
Frontend (src/)  ──invoke/listen──►  Rust (src-tauri/src/)
  views/*.js                           commands.rs     comandos expostos
  api.js (única ponte)                 pipeline.rs     orquestra estágios + eventos
                                       audio/          cpal → WAV 16 kHz (rubato)
                                       whisper.rs      sidecar whisper-cli
                                       gemini.rs       POST + validação do JSON
                                       db.rs           rusqlite + migrations
```

## Pipeline

| Status | Quem define | Significado |
| --- | --- | --- |
| `gravado` | `parar_gravacao` | áudio salvo, aguardando a fila |
| `transcrevendo` | pipeline | whisper-cli em execução |
| `transcrito` | pipeline | texto bruto salvo no banco |
| `resumindo` | pipeline | chamada ao Gemini em andamento |
| `concluido` | pipeline | `ata_gemini` salva (JSON validado) |
| `erro` | pipeline / startup | falhou; `erro` guarda a mensagem |

Eventos para o frontend: `gravacao:nivel` (volume, ~10/s) e `pipeline:etapa`
(`transcricao`, `transcrito`, `ata`, `concluido`, `erro`).

## Decisões que diferem da spec original

- **SQLite via `rusqlite` em vez de `tauri-plugin-sql`.** O pipeline escreve no banco pelo Rust;
  usar o plugin do lado do JS criaria dois escritores, caminhos diferentes (o plugin usa a pasta
  de configuração) e permissões extras. Com o Rust como dono do banco o frontend continua só
  visualizando e disparando ações. As tabelas são as da spec, mais as colunas `status`, `erro`
  e `duracao_seg` (migration 002).
- **Reamostragem em tempo real.** O WAV já é gravado a 16 kHz mono, sem etapa de conversão
  depois (e sem depender de bibliotecas do sistema operacional).
- **Fila única.** O Whisper usa toda a CPU, então as aulas são processadas uma por vez.
