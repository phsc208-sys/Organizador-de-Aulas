use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use crate::error::{AppError, AppResult};

/// Roda o whisper.cpp (sidecar `whisper-cli`) sobre um WAV de 16 kHz mono e devolve o texto corrido.
pub async fn transcrever(
    app: &AppHandle,
    modelo: &Path,
    wav: &Path,
    idioma: &str,
    threads: u32,
) -> AppResult<String> {
    if !modelo.exists() {
        return Err(AppError::Whisper(format!(
            "modelo não encontrado em {}. Rode scripts/download-model.sh.",
            modelo.display()
        )));
    }
    if !wav.exists() {
        return Err(AppError::Whisper(format!("áudio não encontrado: {}", wav.display())));
    }

    // -nt: sem timestamps na saída; -np: só o texto no stdout (logs ficam fora).
    let args: Vec<String> = vec![
        "-m".into(),
        modelo.to_string_lossy().into_owned(),
        "-f".into(),
        wav.to_string_lossy().into_owned(),
        "-l".into(),
        idioma.to_string(),
        "-t".into(),
        threads.max(1).to_string(),
        "-nt".into(),
        "-np".into(),
    ];

    let saida = app
        .shell()
        .sidecar("whisper-cli")
        .map_err(|e| AppError::Whisper(format!("sidecar não encontrado (rode scripts/build-whisper.sh): {e}")))?
        .args(args)
        .output()
        .await
        .map_err(|e| AppError::Whisper(format!("não foi possível executar o whisper-cli: {e}")))?;

    if !saida.status.success() {
        let stderr = String::from_utf8_lossy(&saida.stderr);
        let fim: String = stderr
            .chars()
            .rev()
            .take(500)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return Err(AppError::Whisper(format!(
            "o whisper-cli terminou com erro (código {:?}): {}",
            saida.status.code(),
            fim.trim()
        )));
    }

    let texto = String::from_utf8_lossy(&saida.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if texto.is_empty() {
        return Err(AppError::Whisper(
            "o Whisper não encontrou fala no áudio. Verifique o microfone e o filtro de ruído.".into(),
        ));
    }
    Ok(texto)
}
