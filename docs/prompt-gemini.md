# Prompt de extração (Gemini)

O prompt fica em `src-tauri/prompts/ata.md` e é embutido no binário com `include_str!`
em `src-tauri/src/gemini.rs`. Ele vai como `systemInstruction`; a transcrição segue como
mensagem do usuário.

A saída é forçada para JSON com `responseMimeType` + `responseSchema`. O schema está em
`gemini.rs` (função `schema()`) e precisa ficar igual à struct `Ata`.

## Histórico de versões

- v1: resumo, tópicos principais, datas importantes, tarefas e avisos.
