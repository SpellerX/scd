//! Comandos Tauri do cadastro de empresas (camada fina).
//!
//! Cada função corresponde a um `invoke(...)` do frontend. As regras ficam
//! em `empresas.rs`/`cnpj.rs`/`planilha.rs`; aqui só traduzimos as chamadas.

use tauri::State;

use crate::{cnpj, empresas, AppState};

/// Comando `listar_empresas` — todas as empresas cadastradas.
#[tauri::command]
pub fn listar_empresas(state: State<'_, AppState>) -> Result<Vec<empresas::Empresa>, String> {
    let db = state.db.lock().unwrap();
    empresas::listar(&db)
}

/// Comando `buscar_empresa_por_cnpj` — consulta os dados na BrasilAPI
/// (ainda não salva; o usuário confere e clica em salvar).
#[tauri::command]
pub async fn buscar_empresa_por_cnpj(
    state: State<'_, AppState>,
    cnpj: String,
) -> Result<cnpj::DadosEmpresa, String> {
    let cnpj = cnpj::normalizar_cnpj(&cnpj)?;
    cnpj::buscar_por_cnpj(&state.http, &cnpj).await
}

/// Comando `salvar_empresa` — grava os dados que o usuário conferiu na tela
/// (vindos da busca por CNPJ). NÃO consulta a BrasilAPI de novo: a busca já
/// foi feita uma única vez em `buscar_empresa_por_cnpj`, o que evita gastar
/// o limite de consultas à toa. O backend revalida/padroniza antes de gravar.
#[tauri::command]
pub fn salvar_empresa(
    state: State<'_, AppState>,
    dados: cnpj::DadosEmpresa,
) -> Result<empresas::Empresa, String> {
    let dados = empresas::normalizar_dados(dados)?;

    let db = state.db.lock().unwrap();
    if empresas::existe(&db, &dados.cnpj)? {
        return Err(format!(
            "A empresa com CNPJ {} já está cadastrada.",
            dados.cnpj
        ));
    }
    empresas::inserir(&db, &dados)
}

/// Comando `remover_empresa` — exclui uma empresa cadastrada pelo id.
#[tauri::command]
pub fn remover_empresa(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    empresas::remover(&db, &id)
}

/// Comando `remover_empresas` — exclui várias empresas de uma vez (em lote).
/// `ids` vem das linhas marcadas com checkbox na tela. Empresas com
/// funcionários vinculados são puladas e contadas como "bloqueadas".
#[tauri::command]
pub fn remover_empresas(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<empresas::ResultadoRemocao, String> {
    let mut db = state.db.lock().unwrap();
    empresas::remover_varias(&mut db, &ids)
}

/// Comando `importar_empresas_planilha` — importa em lote de um arquivo
/// .xls/.xlsx escolhido pelo usuário (caminho vem do seletor de arquivos).
///
/// A cada linha processada emite o evento `importacao-progresso`, para o
/// frontend mostrar o andamento (a importação pode demorar quando consulta
/// as fontes públicas). Há também um teto de tempo de segurança: se a
/// importação estourar o limite, o comando retorna e a tela mostra o erro.
#[tauri::command]
pub async fn importar_empresas_planilha(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    caminho: String,
) -> Result<empresas::RelatorioImportacao, String> {
    use tauri::Emitter;

    let linhas = crate::planilha::ler_primeira_aba(&caminho)?;

    if linhas.is_empty() {
        return Err("A planilha está vazia.".to_string());
    }

    // Teto de tempo total (evita que a importação fique "presa" para sempre).
    const TEMPO_MAXIMO: std::time::Duration = std::time::Duration::from_secs(60 * 20);

    let app_para_evento = app.clone();
    let importacao = tokio::time::timeout(
        TEMPO_MAXIMO,
        empresas::importar_linhas(&state.db, &state.http, &linhas, move |progresso| {
            // Reenvia o progresso ao frontend (falha de envio não aborta a importação).
            let _ = app_para_evento.emit("importacao-progresso", progresso);
        }),
    )
    .await;

    match importacao {
        Ok(resultado) => resultado,
        Err(_) => Err(
            "A importação ultrapassou o tempo máximo (20 min) e foi interrompida. \
             Se a planilha é grande e sem razão social, considere dividi-la em partes."
                .to_string(),
        ),
    }
}
