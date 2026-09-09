//! Comandos Tauri de administração de usuários (camada fina).
//! As regras ficam em `auth.rs`; aqui só traduzimos as chamadas do Vue.

use tauri::State;

use crate::{auth, AppState};

/// Comando `listar_usuarios` — todos os usuários cadastrados.
#[tauri::command]
pub fn listar_usuarios(state: State<'_, AppState>) -> Result<Vec<auth::Usuario>, String> {
    let db = state.db.lock().unwrap();
    auth::listar_usuarios(&db)
}

/// Comando `criar_usuario` — cadastra um novo usuário (login da pessoa).
/// Valida nome de exibição, usuário (minúsculas) e senha mínima.
#[tauri::command]
pub fn criar_usuario(
    state: State<'_, AppState>,
    username: String,
    nome_exibicao: String,
    senha: String,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();

    let nome_exibicao = nome_exibicao.trim();
    if nome_exibicao.is_empty() {
        return Err("Informe o nome de exibição do usuário.".to_string());
    }

    let username_normalizado = {
        let digitos_baixo = username.trim().to_lowercase();
        if digitos_baixo.is_empty() {
            return Err("Informe o nome de usuário (login).".to_string());
        }
        digitos_baixo
    };

    auth::create_user(&db, &username_normalizado, nome_exibicao, &senha)
}

/// Comando `remover_usuario` — exclui um usuário pelo id.
/// (O usuário padrão `admin` é protegido no backend.)
#[tauri::command]
pub fn remover_usuario(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    auth::remover_usuario(&db, id)
}

/// Comando `listar_permissoes_do_usuario` — seções (itens de menu) liberadas.
/// Usado pela tela de administração e para montar o menu do usuário logado.
#[tauri::command]
pub fn listar_permissoes_do_usuario(
    state: State<'_, AppState>,
    user_id: i64,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().unwrap();
    auth::listar_secoes_do_usuario(&db, user_id)
}

/// Comando `salvar_permissoes_do_usuario` — define o que o usuário pode ver.
/// `secoes` substitui a lista atual de acessos.
#[tauri::command]
pub fn salvar_permissoes_do_usuario(
    state: State<'_, AppState>,
    user_id: i64,
    secoes: Vec<String>,
) -> Result<(), String> {
    let mut db = state.db.lock().unwrap();
    auth::salvar_secoes_do_usuario(&mut db, user_id, &secoes)
}
