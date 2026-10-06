use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
use tauri::{AppHandle, Emitter, State};

use crate::{
    audio::{capture, TARGET_RATE},
    config::Settings,
    db::{self, Disciplina, DisciplinaInput, RegistroAula, RegistroLista},
    error::{AppError, AppResult},
    pipeline::{self, Etapa},
    state::{travar, ActiveRecording, AppState},
};

// ───────────── Disciplinas ─────────────

#[tauri::command]
pub fn listar_disciplinas(estado: State<'_, AppState>) -> AppResult<Vec<Disciplina>> {
    db::listar_disciplinas(&travar(&estado.db))
}

#[tauri::command]
pub fn criar_disciplina(estado: State<'_, AppState>, dados: DisciplinaInput) -> AppResult<Disciplina> {
    db::criar_disciplina(&travar(&estado.db), &dados)
}

#[tauri::command]
pub fn atualizar_disciplina(
    estado: State<'_, AppState>,
    id: i64,
    dados: DisciplinaInput,
) -> AppResult<Disciplina> {
    db::atualizar_disciplina(&travar(&estado.db), id, &dados)
}

#[tauri::command]
pub fn excluir_disciplina(estado: State<'_, AppState>, id: i64) -> AppResult<()> {
    if matches!(&*travar(&estado.gravacao), Some(g) if g.disciplina_id == id) {
        return Err(AppError::Msg("Pare a gravação em andamento antes de excluir a disciplina.".into()));
    }
    let caminhos = {
        let conn = travar(&estado.db);
        let caminhos = db::caminhos_audio_da_disciplina(&conn, id)?;
        db::excluir_disciplina(&conn, id)?;
        caminhos
    };
    // Arquivos de áudio: remoção em melhor esforço (o registro no banco já foi apagado).
    for c in caminhos {
        let _ = fs::remove_file(c);
    }
    let _ = fs::remove_dir(estado.dirs.audio_dir().join(id.to_string()));
    Ok(())
}

// ───────────── Aulas ─────────────

#[tauri::command]
pub fn listar_registros(estado: State<'_, AppState>, disciplina_id: i64) -> AppResult<Vec<RegistroLista>> {
    db::listar_registros(&travar(&estado.db), disciplina_id)
}

#[tauri::command]
pub fn obter_registro(estado: State<'_, AppState>, id: i64) -> AppResult<RegistroAula> {
    db::obter_registro(&travar(&estado.db), id)
}

fn em_processamento(status: &str) -> bool {
    matches!(status, "transcrevendo" | "resumindo")
}

#[tauri::command]
pub fn excluir_registro(estado: State<'_, AppState>, id: i64) -> AppResult<()> {
    let caminho = {
        let conn = travar(&estado.db);
        let registro = db::obter_registro(&conn, id)?;
        if em_processamento(&registro.status) {
            return Err(AppError::Msg("Aguarde o processamento desta aula terminar.".into()));
        }
        db::excluir_registro(&conn, id)?;
        registro.caminho_audio
    };
    let _ = fs::remove_file(caminho);
    Ok(())
}

/// Roda o pipeline de novo. `desde`: "transcricao" refaz tudo; "ata" reaproveita a transcrição salva.
#[tauri::command]
pub async fn reprocessar_registro(
    app: AppHandle,
    estado: State<'_, AppState>,
    id: i64,
    desde: Option<String>,
) -> AppResult<()> {
    let etapa = {
        let conn = travar(&estado.db);
        let registro = db::obter_registro(&conn, id)?;
        if em_processamento(&registro.status) {
            return Err(AppError::Msg("Esta aula já está sendo processada.".into()));
        }
        let tem_transcricao = registro
            .transcricao_bruta
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);
        let etapa = match desde.as_deref() {
            Some("transcricao") => Etapa::Transcricao,
            Some("ata") => Etapa::Ata,
            _ if tem_transcricao => Etapa::Ata,
            _ => Etapa::Transcricao,
        };
        let status = if etapa == Etapa::Transcricao { "transcrevendo" } else { "resumindo" };
        db::definir_status(&conn, id, status, None)?;
        etapa
    };
    tauri::async_runtime::spawn(pipeline::executar(app, id, etapa));
    Ok(())
}

// ───────────── Gravação ─────────────

#[derive(Serialize)]
pub struct EstadoGravacao {
    gravando: bool,
    disciplina_id: Option<i64>,
    segundos: u64,
}

#[tauri::command]
pub fn estado_gravacao(estado: State<'_, AppState>) -> EstadoGravacao {
    match &*travar(&estado.gravacao) {
        Some(g) => EstadoGravacao {
            gravando: true,
            disciplina_id: Some(g.disciplina_id),
            segundos: g.inicio.elapsed().as_secs(),
        },
        None => EstadoGravacao { gravando: false, disciplina_id: None, segundos: 0 },
    }
}

#[tauri::command]
pub async fn iniciar_gravacao(
    app: AppHandle,
    estado: State<'_, AppState>,
    disciplina_id: i64,
) -> AppResult<()> {
    let mut vaga = travar(&estado.gravacao);
    if vaga.is_some() {
        return Err(AppError::Msg("Já existe uma gravação em andamento.".into()));
    }
    db::obter_disciplina(&travar(&estado.db), disciplina_id)?;

    let settings = Settings::load(&estado.dirs)?;
    let destino = estado
        .dirs
        .tmp_dir()
        .join(format!("{}.wav", Local::now().format("%Y%m%d_%H%M%S")));

    let app_nivel = app.clone();
    let handle = capture::iniciar(
        settings.input_device.clone(),
        settings.pulse_source.clone(),
        destino,
        move |nivel| {
            let _ = app_nivel.emit("gravacao:nivel", nivel);
        },
    )?;

    *vaga = Some(ActiveRecording { handle, disciplina_id, inicio: Instant::now() });
    Ok(())
}

/// Para a gravação, guarda o áudio, cria o registro e dispara o pipeline em segundo plano.
#[tauri::command]
pub async fn parar_gravacao(app: AppHandle, estado: State<'_, AppState>) -> AppResult<i64> {
    let ativa = travar(&estado.gravacao)
        .take()
        .ok_or_else(|| AppError::Msg("Nenhuma gravação em andamento.".into()))?;
    let disciplina_id = ativa.disciplina_id;

    let (tmp, amostras) = ativa.handle.parar()?;
    let segundos = (amostras / TARGET_RATE as u64) as i64;
    if segundos < 2 {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::Msg("Gravação com menos de 2 segundos: nada foi salvo.".into()));
    }

    let pasta = estado.dirs.audio_dir().join(disciplina_id.to_string());
    fs::create_dir_all(&pasta)?;
    let final_path = pasta.join(format!("{}.wav", Local::now().format("%Y-%m-%d_%H%M%S")));
    mover(&tmp, &final_path)?;

    let id = {
        let conn = travar(&estado.db);
        db::inserir_registro(&conn, disciplina_id, &final_path.to_string_lossy(), segundos)?
    };
    tauri::async_runtime::spawn(pipeline::executar(app, id, Etapa::Transcricao));
    Ok(id)
}

fn mover(origem: &Path, destino: &Path) -> AppResult<()> {
    if fs::rename(origem, destino).is_ok() {
        return Ok(());
    }
    // rename falha entre sistemas de arquivos diferentes: copia e apaga.
    fs::copy(origem, destino)?;
    fs::remove_file(origem)?;
    Ok(())
}

#[tauri::command]
pub async fn listar_dispositivos_entrada() -> AppResult<Vec<String>> {
    capture::listar_dispositivos()
}

// ───────────── Configurações ─────────────

#[derive(Serialize)]
pub struct ConfiguracoesView {
    gemini_model: String,
    whisper_model: String,
    whisper_language: String,
    whisper_threads: u32,
    input_device: String,
    pulse_source: String,
    gemini_api_key_definida: bool,
}

#[derive(Deserialize)]
pub struct ConfiguracoesInput {
    /// None ou vazio = mantém a chave atual.
    gemini_api_key: Option<String>,
    gemini_model: String,
    whisper_model: String,
    whisper_language: String,
    whisper_threads: u32,
    input_device: String,
    pulse_source: String,
}

fn visao(s: &Settings) -> ConfiguracoesView {
    ConfiguracoesView {
        gemini_model: s.gemini_model.clone(),
        whisper_model: s.whisper_model.clone(),
        whisper_language: s.whisper_language.clone(),
        whisper_threads: s.whisper_threads,
        input_device: s.input_device.clone(),
        pulse_source: s.pulse_source.clone(),
        gemini_api_key_definida: s.gemini_key().is_some(),
    }
}

#[tauri::command]
pub fn obter_configuracoes(estado: State<'_, AppState>) -> AppResult<ConfiguracoesView> {
    Ok(visao(&Settings::load(&estado.dirs)?))
}

#[tauri::command]
pub fn salvar_configuracoes(
    estado: State<'_, AppState>,
    dados: ConfiguracoesInput,
) -> AppResult<ConfiguracoesView> {
    let modelo = dados.whisper_model.trim().to_string();
    if modelo.is_empty() || Path::new(&modelo).file_name().and_then(|n| n.to_str()) != Some(modelo.as_str()) {
        return Err(AppError::Config(
            "o modelo do Whisper deve ser só o nome do arquivo (ex.: ggml-small.bin).".into(),
        ));
    }
    if dados.gemini_model.trim().is_empty() {
        return Err(AppError::Config("informe o modelo do Gemini.".into()));
    }
    if dados.whisper_language.trim().is_empty() {
        return Err(AppError::Config("informe o idioma do Whisper (ex.: pt).".into()));
    }
    if !(1..=64).contains(&dados.whisper_threads) {
        return Err(AppError::Config("o número de threads deve ficar entre 1 e 64.".into()));
    }

    let mut s = Settings::load(&estado.dirs)?;
    if let Some(chave) = dados.gemini_api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        s.gemini_api_key = chave.to_string();
    }
    s.gemini_model = dados.gemini_model.trim().to_string();
    s.whisper_model = modelo;
    s.whisper_language = dados.whisper_language.trim().to_string();
    s.whisper_threads = dados.whisper_threads;
    s.input_device = dados.input_device.trim().to_string();
    s.pulse_source = dados.pulse_source.trim().to_string();
    s.save(&estado.dirs)?;
    Ok(visao(&s))
}

#[derive(Serialize)]
pub struct Ambiente {
    modelo_whisper_ok: bool,
    caminho_modelo: PathBuf,
    gemini_key_ok: bool,
}

/// Checagem rápida para a interface avisar o que falta antes da primeira gravação.
#[tauri::command]
pub fn verificar_ambiente(estado: State<'_, AppState>) -> AppResult<Ambiente> {
    let s = Settings::load(&estado.dirs)?;
    let caminho_modelo = estado.dirs.models_dir().join(&s.whisper_model);
    Ok(Ambiente {
        modelo_whisper_ok: caminho_modelo.exists(),
        caminho_modelo,
        gemini_key_ok: s.gemini_key().is_some(),
    })
}
