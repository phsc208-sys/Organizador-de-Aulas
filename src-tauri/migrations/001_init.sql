CREATE TABLE disciplinas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nome TEXT NOT NULL,
    professor TEXT,
    semestre TEXT,
    carga_horaria INTEGER
);

CREATE TABLE registros_aula (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    disciplina_id INTEGER NOT NULL,
    data_hora DATETIME DEFAULT CURRENT_TIMESTAMP,
    caminho_audio TEXT NOT NULL,
    transcricao_bruta TEXT,
    ata_gemini TEXT,
    FOREIGN KEY(disciplina_id) REFERENCES disciplinas(id) ON DELETE CASCADE
);

CREATE INDEX idx_registros_disciplina ON registros_aula(disciplina_id, data_hora DESC);
