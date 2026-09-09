//! Comandos Tauri da sincronização com a nuvem (camada fina).
//! As regras ficam em `sync.rs`; aqui só traduzimos as chamadas do Vue.

use tauri::State;

use crate::{sync, AppState};

/// Comando `estado_sync` — situação da conta da nuvem neste dispositivo.
#[tauri::command]
pub fn estado_sync(state: State<'_, AppState>) -> Result<sync::EstadoSync, String> {
    sync::estado(&state.db)
}

/// Comando `conectar_sync` — valida e guarda a conta da nuvem (GoTrue).
#[tauri::command]
pub async fn conectar_sync(
    state: State<'_, AppState>,
    config: sync::ConfiguracaoConexao,
) -> Result<(), String> {
    sync::conectar(&state.db, &state.http, config).await
}

/// Comando `desconectar_sync` — remove a conta e os marcadores do dispositivo.
#[tauri::command]
pub fn desconectar_sync(state: State<'_, AppState>) -> Result<(), String> {
    sync::desconectar(&state.db)
}

/// Comando `sincronizar_agora` — executa um ciclo completo (push + pull).
#[tauri::command]
pub async fn sincronizar_agora(state: State<'_, AppState>) -> Result<sync::ResumoSync, String> {
    sync::sincronizar(&state.db, &state.http).await
}
