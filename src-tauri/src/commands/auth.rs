//! Comandos Tauri de autenticação (camada fina de entrada/saída).
//!
//! Cada função deste arquivo corresponde a um `invoke(...)` feito no Vue.
//! A regra de negócio (conferir senha, criar sessão) fica em `auth.rs`.
//!
//! Os comandos são `async` para rodarem fora da thread da interface
//! (bcrypt/SQLite são bloqueantes por alguns milissegundos).

use tauri::State;

use crate::{auth, AppState};

/// Comando `login` — valida usuário/senha e inicia a sessão.
/// Com `lembrar: true`, cria uma sessão persistente e devolve o token
/// (o app guarda o token para abrir já logado na próxima vez).
#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    username: String,
    password: String,
    lembrar: bool,
) -> Result<auth::RespostaLogin, String> {
    let db = state.db.lock().unwrap();

    let user = auth::authenticate(&db, username.trim(), &password)?
        .ok_or_else(|| "Usuário ou senha inválidos.".to_string())?;

    let token = if lembrar {
        Some(auth::criar_sessao(&db, user.id)?)
    } else {
        None
    };

    Ok(auth::RespostaLogin { user, token })
}

/// Comando `logout` — encerra a sessão persistente (quando houver token)
/// e desloga no app.
#[tauri::command]
pub fn logout(state: State<'_, AppState>, token: Option<String>) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    if let Some(token) = token {
        auth::encerrar_sessao(&db, &token)?;
    }
    Ok(())
}

/// Comando `autenticar_por_token` — restaura a sessão ao abrir o aplicativo.
/// Recebe o token salvo pelo login com "Manter-me conectado" e devolve o
/// usuário correspondente (`null` se o token não for mais válido).
#[tauri::command]
pub fn autenticar_por_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<Option<auth::User>, String> {
    let db = state.db.lock().unwrap();
    auth::usuario_por_token(&db, &token)
}
