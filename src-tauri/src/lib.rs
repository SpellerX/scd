// Ponto de montagem do aplicativo Tauri (chamado pelo main.rs).
// Aqui ficam: estado global, inicialização (banco de dados) e o registro
// de comandos expostos ao frontend.

mod auth;
mod cnpj;
mod commands;
mod db;
mod empresas;
mod ferias;
mod funcionarios;
mod planilha;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

/// Estado global compartilhado entre os comandos Tauri.
pub struct AppState {
    /// Conexão com o banco SQLite local. `Connection` não é `Sync`, por isso
    /// fica num `Mutex` — os comandos rodam em threads diferentes.
    pub db: Mutex<Connection>,
    /// Cliente HTTP compartilhado (usado nas consultas de CNPJ).
    pub http: reqwest::Client,
    /// Pendência atualmente exibida na janela própria de notificação.
    /// A página `noti.html` busca por comando (sem dados na URL).
    pub pendencia_notificacao: Mutex<Option<crate::ferias::PendenciaNotificacao>>,
    /// A página de notificação avisou que terminou de carregar?
    /// Usado pelo vigia de segurança para fechar janela travada.
    pub janela_notificacao_pronta: Mutex<bool>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Instância única: abrir o app de novo foca a janela existente em vez
        // de criar uma segunda instância (evita janelas duplicadas).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // 1) Resolve o diretório de dados do app e abre o banco SQLite.
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("não foi possível resolver o diretório de dados do app");
            std::fs::create_dir_all(&data_dir)
                .expect("não foi possível criar o diretório de dados do app");
            let conn = match db::open(&data_dir.join("scd.db")) {
                Ok(conn) => conn,
                Err(motivo) => {
                    // Registra o motivo (ex.: banco em versão mais nova que o
                    // app) antes de abortar — diagnóstico sem abrir o app.
                    eprintln!("[scd] erro ao abrir o banco de dados: {motivo}");
                    panic!("não foi possível abrir o banco de dados: {motivo}");
                }
            };

            // 2) Garante o usuário de demonstração (apenas desenvolvimento).
            auth::ensure_demo_user(&conn).expect("não foi possível criar o usuário demo");

            // 3) Gera períodos de férias em falta dos funcionários já existentes.
            ferias::gerar_periodos_existentes(&conn)
                .expect("não foi possível gerar os períodos de férias");

            // 4) Disponibiliza o estado para os comandos.
            app.manage(AppState {
                db: Mutex::new(conn),
                pendencia_notificacao: Mutex::new(None),
                janela_notificacao_pronta: Mutex::new(false),
                http: reqwest::Client::builder()
                    // Tempo máximo para estabelecer a conexão e para cada
                    // requisição (consulta de CNPJ não pode travar a tela).
                    .connect_timeout(std::time::Duration::from_secs(8))
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .expect("não foi possível criar o cliente HTTP"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Para expor um novo comando ao frontend, registre-o aqui:
            // commands::<area>::<nome_do_comando>
            commands::auth::login,
            commands::auth::logout,
            commands::auth::autenticar_por_token,
            commands::empresas::listar_empresas,
            commands::empresas::buscar_empresa_por_cnpj,
            commands::empresas::salvar_empresa,
            commands::empresas::remover_empresa,
            commands::empresas::remover_empresas,
            commands::empresas::importar_empresas_planilha,
            commands::funcionarios::listar_funcionarios,
            commands::funcionarios::criar_funcionario,
            commands::funcionarios::importar_funcionarios_planilha,
            commands::funcionarios::remover_funcionario,
            commands::usuarios::listar_usuarios,
            commands::usuarios::criar_usuario,
            commands::usuarios::remover_usuario,
            commands::usuarios::listar_permissoes_do_usuario,
            commands::usuarios::salvar_permissoes_do_usuario,
            commands::ferias::listar_ferias_vencidas,
            commands::ferias::listar_ferias_a_vencer,
            commands::ferias::regularizar_ferias,
            commands::ferias::obter_alerta_ferias,
            commands::ferias::definir_alerta_ferias,
            commands::ferias::listar_notificacoes,
            commands::ferias::verificar_notificacoes_os,
            commands::ferias::sonecar_notificacoes_os,
            commands::ferias::acao_notificacao,
            commands::ferias::obter_pendencia_notificacao,
            commands::ferias::notificacao_janela_pronta,
            commands::ferias::fechar_janela_notificacao
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o aplicativo Tauri");
}
