use rusqlite::Connection;
use std::{
    sync::{Mutex, MutexGuard},
    time::Instant,
};

use crate::{audio::capture::RecordingHandle, config::Dirs};

pub struct ActiveRecording {
    pub handle: RecordingHandle,
    pub disciplina_id: i64,
    pub inicio: Instant,
}

pub struct AppState {
    pub dirs: Dirs,
    pub db: Mutex<Connection>,
    pub gravacao: Mutex<Option<ActiveRecording>>,
    /// Serializa o pipeline: só uma aula é transcrita/resumida por vez (o Whisper usa toda a CPU).
    pub fila: tokio::sync::Mutex<()>,
}

impl AppState {
    pub fn novo(dirs: Dirs, conn: Connection) -> Self {
        Self {
            dirs,
            db: Mutex::new(conn),
            gravacao: Mutex::new(None),
            fila: tokio::sync::Mutex::new(()),
        }
    }
}

/// Lock que ignora envenenamento: um panic em outra thread não deve travar o app inteiro.
pub fn travar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
