use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    config::Settings,
    db,
    error::AppResult,
    gemini,
    state::{travar, AppState},
    whisper,
};

/// Estágio de onde o pipeline recomeça.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etapa {
    Transcricao,
    Ata,
}

#[derive(Clone, Serialize)]
struct EventoEtapa {
    registro_id: i64,
    /// transcricao | transcrito | ata | concluido | erro
    etapa: &'static str,
    mensagem: Option<String>,
}

fn emitir(app: &AppHandle, registro_id: i64, etapa: &'static str, mensagem: Option<String>) {
    let _ = app.emit("pipeline:etapa", EventoEtapa { registro_id, etapa, mensagem });
}

/// Ponto de entrada do pipeline. Nunca devolve erro: falhas viram status `erro` no banco + evento.
pub async fn executar(app: AppHandle, registro_id: i64, desde: Etapa) {
    if let Err(e) = rodar(&app, registro_id, desde).await {
        let mensagem = e.to_string();
        {
            let estado = app.state::<AppState>();
            let conn = travar(&estado.db);
            let _ = db::definir_status(&conn, registro_id, "erro", Some(&mensagem));
        }
        emitir(&app, registro_id, "erro", Some(mensagem));
    }
}

async fn rodar(app: &AppHandle, registro_id: i64, desde: Etapa) -> AppResult<()> {
    let estado = app.state::<AppState>();
    // Espera a vez: o Whisper ocupa a CPU toda, então as aulas são processadas uma por vez.
    let _vez = estado.fila.lock().await;

    let settings = Settings::load(&estado.dirs)?;
    let (registro, disciplina) = {
        let conn = travar(&estado.db);
        let registro = db::obter_registro(&conn, registro_id)?;
        let disciplina = db::obter_disciplina(&conn, registro.disciplina_id)?;
        (registro, disciplina)
    };

    let mut transcricao = registro.transcricao_bruta.clone().unwrap_or_default();

    // Estágio 1: Whisper (offline). Se a transcrição já existe e só a ata falhou, é pulado.
    if desde == Etapa::Transcricao || transcricao.trim().is_empty() {
        {
            let conn = travar(&estado.db);
            db::definir_status(&conn, registro_id, "transcrevendo", None)?;
        }
        emitir(app, registro_id, "transcricao", None);

        let modelo = estado.dirs.models_dir().join(&settings.whisper_model);
        transcricao = whisper::transcrever(
            app,
            &modelo,
            Path::new(&registro.caminho_audio),
            &settings.whisper_language,
            settings.whisper_threads,
        )
        .await?;

        {
            // A partir daqui o texto bruto está salvo: uma falha de rede no Gemini não perde mais nada.
            let conn = travar(&estado.db);
            db::salvar_transcricao(&conn, registro_id, &transcricao)?;
        }
        emitir(app, registro_id, "transcrito", None);
    }

    // Estágio 2: Gemini.
    {
        let conn = travar(&estado.db);
        db::definir_status(&conn, registro_id, "resumindo", None)?;
    }
    emitir(app, registro_id, "ata", None);

    let ata = gemini::gerar_ata(&settings, &transcricao, &disciplina.nome).await?;

    {
        let conn = travar(&estado.db);
        db::salvar_ata(&conn, registro_id, &ata)?;
    }
    emitir(app, registro_id, "concluido", None);
    Ok(())
}
