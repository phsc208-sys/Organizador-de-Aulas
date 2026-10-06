mod audio;
mod commands;
mod config;
mod db;
mod error;
mod gemini;
mod pipeline;
mod state;
mod whisper;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let dirs = config::Dirs {
                data: app.path().app_data_dir()?,
                config: app.path().app_config_dir()?,
            };
            dirs.garantir()?;

            let conn = db::abrir(&dirs.db_path())?;
            db::recuperar_interrompidos(&conn)?;
            app.manage(state::AppState::novo(dirs, conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::listar_disciplinas,
            commands::criar_disciplina,
            commands::atualizar_disciplina,
            commands::excluir_disciplina,
            commands::listar_registros,
            commands::obter_registro,
            commands::excluir_registro,
            commands::reprocessar_registro,
            commands::estado_gravacao,
            commands::iniciar_gravacao,
            commands::parar_gravacao,
            commands::listar_dispositivos_entrada,
            commands::obter_configuracoes,
            commands::salvar_configuracoes,
            commands::verificar_ambiente,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Transcritor de Aulas");
}
