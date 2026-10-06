use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Banco de dados: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Áudio: {0}")]
    Audio(String),
    #[error("Whisper: {0}")]
    Whisper(String),
    #[error("Gemini: {0}")]
    Gemini(String),
    #[error("Configuração: {0}")]
    Config(String),
    #[error("{0}")]
    Msg(String),
}

// O Tauri exige que o erro de um comando seja serializável; o frontend recebe a mensagem como texto.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
