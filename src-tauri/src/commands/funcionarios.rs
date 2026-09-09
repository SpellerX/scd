//! Comandos Tauri de funcionários (camada fina).
//! As regras ficam em `funcionarios.rs`; aqui só traduzimos as chamadas do Vue.

use tauri::State;

use crate::{funcionarios, AppState};

/// Comando `listar_funcionarios` — todos os funcionários (com a empresa).
#[tauri::command]
pub fn listar_funcionarios(
    state: State<'_, AppState>,
) -> Result<Vec<funcionarios::Funcionario>, String> {
    let db = state.db.lock().unwrap();
    funcionarios::listar(&db)
}

/// Comando `criar_funcionario` — cadastra e vincula um funcionário a uma empresa.
/// O backend valida nome, empresa existente e CPF (quando informado).
#[tauri::command]
pub fn criar_funcionario(
    state: State<'_, AppState>,
    nome: String,
    cpf: String,
    empresa_id: String,
    data_admissao: Option<String>,
) -> Result<funcionarios::Funcionario, String> {
    let db = state.db.lock().unwrap();
    funcionarios::inserir(&db, &nome, &cpf, &empresa_id, data_admissao)
}

/// Comando `importar_funcionarios_planilha` — importa funcionários de um
/// .xls/.xlsx. `empresa_id` é a empresa padrão (usada quando a planilha não
/// tem coluna de CNPJ por linha ou a linha está sem CNPJ).
#[tauri::command]
pub fn importar_funcionarios_planilha(
    state: State<'_, AppState>,
    caminho: String,
    empresa_id: Option<String>,
) -> Result<funcionarios::RelatorioImportacaoFuncionarios, String> {
    let linhas = crate::planilha::ler_primeira_aba(&caminho)?;

    if linhas.is_empty() {
        return Err("A planilha está vazia.".to_string());
    }

    let db = state.db.lock().unwrap();
    funcionarios::importar_linhas(&db, &linhas, empresa_id)
}

/// Comando `remover_funcionario` — exclui um funcionário pelo id.
#[tauri::command]
pub fn remover_funcionario(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    funcionarios::remover(&db, &id)
}
