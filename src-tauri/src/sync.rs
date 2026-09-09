//! Motor de sincronização local ↔ nuvem (Supabase) do SCD.
//!
//! Modelo (aprovado em conjunto com o usuário):
//! - o **SQLite local continua sendo a fonte de leitura/escrita da UI**;
//! - uma **conta "dona"** (e-mail/senha no Supabase Auth/GoTrue) sincroniza os
//!   dados de uma empresa; o app guarda as credenciais/token no dispositivo;
//! - sincronizam-se as tabelas `companies`, `employees` e
//!   `employee_leave_periods` (ids UUID + `updated_at` + `deleted_at`);
//! - conflito: **last-write-wins** por `updated_at` (normalizado em UTC);
//!   em empate, vence a nuvem (fonte autoritativa);
//! - a cada ciclo: 1) **push** dos registros locais alterados desde o último
//!   envio (upsert via PostgREST); 2) **pull** dos deltas da nuvem
//!   (`updated_at > última puxada`), aplicando apenas os registros mais novos.
//!
//! As credenciais e os marcadores ficam na tabela local `app_settings`
//! (nunca sincronizada), sob as chaves `sync.*`.

use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

// ═══════════════════ Chaves de configuração (app_settings) ═══════════════════

const CHAVE_PROJETO_URL: &str = "sync.projeto_url";
const CHAVE_ANON_KEY: &str = "sync.anon_key";
const CHAVE_EMAIL: &str = "sync.email";
const CHAVE_SENHA: &str = "sync.senha";
const CHAVE_REFRESH_TOKEN: &str = "sync.refresh_token";
const CHAVE_ACCESS_TOKEN: &str = "sync.access_token";
/// Registros locais com `updated_at` acima deste valor ainda não foram
/// enviados à nuvem.
const CHAVE_PUSH_ATE: &str = "sync.push_ate";
/// Registros remotos com `updated_at` acima deste valor ainda não foram
/// trazidos para o banco local.
const CHAVE_PULL_ATE: &str = "sync.pull_ate";
/// Última sincronização bem-sucedida (exibida na tela).
const CHAVE_ULTIMA_SYNC: &str = "sync.ultima_sync_em";

/// Tabelas sincronizadas, na ordem de dependência (FK). `users` não depende
/// de nenhuma (os acessos por módulo viajam embutidos no próprio registro).
const TABELAS: [&str; 4] = ["users", "companies", "employees", "employee_leave_periods"];

// ═══════════════════ Estruturas expostas ao frontend ═════════════════════════

/// Estado atual da sincronização (tela Sistema → Sincronização).
#[derive(Debug, Clone, Serialize)]
pub struct EstadoSync {
    /// Há uma conta de nuvem configurada neste dispositivo?
    pub conectado: bool,
    /// E-mail da conta configurada.
    pub email: Option<String>,
    /// Momento da última sincronização bem-sucedida.
    pub ultima_sync: Option<String>,
}

/// Resumo de um ciclo de sincronização.
#[derive(Debug, Clone, Serialize)]
pub struct ResumoSync {
    pub enviados: usize,
    pub recebidos: usize,
    /// Problemas não fatais (ex.: registro cujo vínculo ainda não chegou —
    /// é resolvido no próximo ciclo).
    pub avisos: Vec<String>,
}

/// Dados necessários para conectar a conta da nuvem.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfiguracaoConexao {
    pub projeto_url: String,
    pub anon_key: String,
    pub email: String,
    pub senha: String,
}

/// Carrega o arquivo `.env` — procura no diretório atual e nos pais (cobre
/// rodar o app a partir da raiz ou de `src-tauri`). Falhas são ignoradas:
/// `.env` é só um facilitador de desenvolvimento/teste.
fn carregar_arquivo_env() {
    let Ok(atual) = std::env::current_dir() else {
        return;
    };
    for dir in atual.ancestors().take(4) {
        let caminho = dir.join(".env");
        if caminho.exists() {
            let _ = dotenvy::from_path(&caminho);
            return;
        }
    }
}

/// Lê a configuração da nuvem das variáveis de ambiente (arquivo `.env`),
/// devolvendo `None` quando alguma parte essencial está ausente.
fn carregar_env() -> Option<ConfiguracaoConexao> {
    carregar_arquivo_env();
    let var = |nome: &str| -> Option<String> {
        let valor = std::env::var(nome).ok()?;
        let valor = valor.trim().to_string();
        if valor.is_empty() {
            None
        } else {
            Some(valor)
        }
    };
    Some(ConfiguracaoConexao {
        projeto_url: var("SCD_SUPABASE_URL")?,
        anon_key: var("SCD_SUPABASE_ANON_KEY")?,
        email: var("SCD_SUPABASE_EMAIL")?,
        senha: var("SCD_SUPABASE_SENHA")?,
    })
}

/// Configuração EMBUTIDA no binário no momento do build (produção: instalar e
/// usar sem digitar nada). Defina as variáveis `SCD_EMBUTIDO_*` no ambiente de
/// compilação, ex.:
///   $env:SCD_EMBUTIDO_SUPABASE_URL="https://..."; npm run tauri build
///
/// A conta dona embutida é compartilhada por todas as máquinas da empresa —
/// só faça isso em builds de distribuição PRIVADA (a proteção dos dados passa
/// a depender do sigilo do instalador). Para build público, mantenha estas
/// variáveis vazias e deixe cada máquina conectar pela tela.
fn config_embutida() -> Option<ConfiguracaoConexao> {
    let var = |valor: Option<&'static str>| -> Option<String> {
        let valor = valor?.trim().to_string();
        if valor.is_empty() {
            None
        } else {
            Some(valor)
        }
    };
    Some(ConfiguracaoConexao {
        projeto_url: var(option_env!("SCD_EMBUTIDO_SUPABASE_URL"))?,
        anon_key: var(option_env!("SCD_EMBUTIDO_SUPABASE_ANON_KEY"))?,
        email: var(option_env!("SCD_EMBUTIDO_SUPABASE_EMAIL"))?,
        senha: var(option_env!("SCD_EMBUTIDO_SUPABASE_SENHA"))?,
    })
}

// ═══════════════════ Helpers de app_settings ═════════════════════════════════

fn ler_config(conn: &Connection, chave: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT valor FROM app_settings WHERE chave = ?1",
        [chave],
        |linha| linha.get(0),
    )
    .optional()
    .map_err(|err| format!("Falha ao ler a configuração de sincronização: {err}"))
}

fn salvar_config(conn: &Connection, chave: &str, valor: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (chave, valor) VALUES (?1, ?2)
         ON CONFLICT(chave) DO UPDATE SET valor = excluded.valor",
        params![chave, valor],
    )
    .map_err(|err| format!("Falha ao salvar a configuração de sincronização: {err}"))?;
    Ok(())
}

fn remover_config(conn: &Connection, chave: &str) -> Result<(), String> {
    conn.execute("DELETE FROM app_settings WHERE chave = ?1", [chave])
        .map_err(|err| format!("Falha ao remover a configuração de sincronização: {err}"))?;
    Ok(())
}

/// "agora" no formato local (UTC, `YYYY-MM-DD HH:MM:SS`).
fn agora_sqlite(conn: &Connection) -> Result<String, String> {
    conn.query_row("SELECT datetime('now')", [], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao ler a data atual: {err}"))
}

/// Normaliza um timestamp vindo do Postgres (`timestamptz`) para o formato
/// local (`YYYY-MM-DD HH:MM:SS`, UTC) — permitindo comparação textual.
/// Ex.: `2025-05-05T12:34:56.789123+00:00` → `2025-05-05 12:34:56`.
fn normalizar_ts_servidor(valor: &str) -> Option<String> {
    let valor = valor.trim();
    if valor.len() < 19 {
        return None;
    }
    // Os 19 primeiros caracteres já são "YYYY-MM-DDTHH:MM:SS".
    Some(valor[..19].replace('T', " "))
}

/// Lê um campo string do JSON remoto (ausente/nulo → `None`).
fn campo_str(registro: &serde_json::Map<String, serde_json::Value>, nome: &str) -> Option<String> {
    registro.get(nome).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Null => None,
        _ => None,
    })
}

/// Id (UUID) de um registro remoto.
fn id_do_registro(registro: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    campo_str(registro, "id")
}

// ═══════════════════ Autenticação (GoTrue) ══════════════════════════════════

/// Troca e-mail/senha por um par de tokens (access + refresh) no GoTrue.
async fn login_gotrue(
    http: &reqwest::Client,
    cfg: &ConfiguracaoConexao,
) -> Result<(String, String), String> {
    let corpo_envio = serde_json::to_string(&serde_json::json!({
        "email": cfg.email.trim(),
        "password": cfg.senha,
    }))
    .map_err(|err| format!("Falha ao montar o login: {err}"))?;

    let resposta = http
        .post(format!(
            "{}/auth/v1/token?grant_type=password",
            cfg.projeto_url.trim_end_matches('/')
        ))
        .header("apikey", cfg.anon_key.trim())
        .header("Content-Type", "application/json")
        .body(corpo_envio)
        .send()
        .await
        .map_err(|err| format!("Não foi possível conectar ao Supabase: {err}"))?;

    let corpo = resposta
        .text()
        .await
        .map_err(|err| format!("Falha ao ler a resposta do Supabase: {err}"))?;
    let json: serde_json::Value = serde_json::from_str(&corpo)
        .map_err(|err| format!("Resposta inválida do Supabase: {err}"))?;

    let acesso = json.get("access_token").and_then(|v| v.as_str());
    let refresh = json.get("refresh_token").and_then(|v| v.as_str());
    match (acesso, refresh) {
        (Some(acesso), Some(refresh)) => Ok((acesso.to_string(), refresh.to_string())),
        _ => {
            let detalhe = json
                .get("error_description")
                .or_else(|| json.get("msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("credenciais inválidas");
            Err(format!("Falha ao entrar na conta da nuvem: {detalhe}"))
        }
    }
}

/// Renova o access token usando o refresh token (rotação de tokens).
async fn renovar_gotrue(
    http: &reqwest::Client,
    cfg: &ConfiguracaoConexao,
    refresh_token: &str,
) -> Result<(String, String), String> {
    let corpo_envio = serde_json::to_string(&serde_json::json!({
        "refresh_token": refresh_token
    }))
    .map_err(|err| format!("Falha ao montar a renovação: {err}"))?;

    let resposta = http
        .post(format!(
            "{}/auth/v1/token?grant_type=refresh_token",
            cfg.projeto_url.trim_end_matches('/')
        ))
        .header("apikey", cfg.anon_key.trim())
        .header("Content-Type", "application/json")
        .body(corpo_envio)
        .send()
        .await
        .map_err(|err| format!("Não foi possível renovar a sessão da nuvem: {err}"))?;

    let corpo = resposta
        .text()
        .await
        .map_err(|err| format!("Falha ao ler a resposta do Supabase: {err}"))?;
    let json: serde_json::Value = serde_json::from_str(&corpo)
        .map_err(|err| format!("Resposta inválida do Supabase: {err}"))?;

    let acesso = json.get("access_token").and_then(|v| v.as_str());
    let refresh = json.get("refresh_token").and_then(|v| v.as_str());
    match (acesso, refresh) {
        (Some(acesso), Some(refresh)) => Ok((acesso.to_string(), refresh.to_string())),
        _ => Err("A sessão da nuvem expirou. Entre novamente com e-mail e senha.".to_string()),
    }
}

/// Valida o access token (GET /auth/v1/user) e devolve o uid do dono.
async fn uid_do_token(
    http: &reqwest::Client,
    cfg: &ConfiguracaoConexao,
    access_token: &str,
) -> Result<String, String> {
    let resposta = http
        .get(format!(
            "{}/auth/v1/user",
            cfg.projeto_url.trim_end_matches('/')
        ))
        .header("apikey", cfg.anon_key.trim())
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|err| format!("Não foi possível validar a sessão da nuvem: {err}"))?;

    let corpo = resposta
        .text()
        .await
        .map_err(|err| format!("Falha ao ler a resposta do Supabase: {err}"))?;
    let json: serde_json::Value = serde_json::from_str(&corpo)
        .map_err(|err| format!("Resposta inválida do Supabase: {err}"))?;

    json.get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "Sessão da nuvem inválida. Entre novamente com e-mail e senha.".to_string())
}

// ═══════════════════ API pública (usada pelos comandos) ══════════════════════

/// Estado atual da configuração de sincronização.
pub fn estado(db: &Mutex<Connection>) -> Result<EstadoSync, String> {
    let conn = db.lock().unwrap();
    Ok(EstadoSync {
        conectado: ler_config(&conn, CHAVE_REFRESH_TOKEN)?.is_some(),
        email: ler_config(&conn, CHAVE_EMAIL)?,
        ultima_sync: ler_config(&conn, CHAVE_ULTIMA_SYNC)?,
    })
}

/// Conecta a conta da nuvem (login GoTrue) e guarda as credenciais.
pub async fn conectar(
    db: &Mutex<Connection>,
    http: &reqwest::Client,
    cfg: ConfiguracaoConexao,
) -> Result<(), String> {
    // Login ANTES de persistir qualquer coisa (não guarda credencial inválida).
    let (acesso, refresh) = login_gotrue(http, &cfg).await?;
    let conn = db.lock().unwrap();
    for (chave, valor) in [
        (CHAVE_PROJETO_URL, cfg.projeto_url.trim().to_string()),
        (CHAVE_ANON_KEY, cfg.anon_key.trim().to_string()),
        (CHAVE_EMAIL, cfg.email.trim().to_string()),
        (CHAVE_SENHA, cfg.senha),
        (CHAVE_ACCESS_TOKEN, acesso),
        (CHAVE_REFRESH_TOKEN, refresh),
    ] {
        salvar_config(&conn, chave, &valor)?;
    }
    Ok(())
}

/// Desconecta a conta da nuvem (remove credenciais e marcadores).
pub fn desconectar(db: &Mutex<Connection>) -> Result<(), String> {
    let conn = db.lock().unwrap();
    for chave in [
        CHAVE_PROJETO_URL,
        CHAVE_ANON_KEY,
        CHAVE_EMAIL,
        CHAVE_SENHA,
        CHAVE_ACCESS_TOKEN,
        CHAVE_REFRESH_TOKEN,
        CHAVE_PUSH_ATE,
        CHAVE_PULL_ATE,
        CHAVE_ULTIMA_SYNC,
    ] {
        remover_config(&conn, chave)?;
    }
    Ok(())
}

/// Executa um ciclo completo de sincronização (push + pull).
pub async fn sincronizar(
    db: &Mutex<Connection>,
    http: &reqwest::Client,
) -> Result<ResumoSync, String> {
    let mut resumo = ResumoSync {
        enviados: 0,
        recebidos: 0,
        avisos: Vec::new(),
    };

    // 1) Configuração salva no dispositivo. Se ainda não há conta conectada,
    //    tenta provisionar a partir do arquivo `.env` (desenvolvimento/teste).
    let (url, anon) = {
        let conn = db.lock().unwrap();
        let url = ler_config(&conn, CHAVE_PROJETO_URL)?;
        let anon = ler_config(&conn, CHAVE_ANON_KEY)?;
        (url, anon)
    };

    let (url, anon, email, refresh) = match (url, anon) {
        (Some(url), Some(anon)) => {
            let conn = db.lock().unwrap();
            let email = ler_config(&conn, CHAVE_EMAIL)?.unwrap_or_default();
            let refresh = ler_config(&conn, CHAVE_REFRESH_TOKEN)?.unwrap_or_default();
            (url, anon, email, refresh)
        }
        // Nada salvo ainda → usa o .env se estiver completo (dev/teste) ou a
        // configuração embutida no build (instalador pronto para usar).
        _ => {
            if let Some(cfg_env) = carregar_env().or_else(config_embutida) {
                conectar(db, http, cfg_env).await?;
                let conn = db.lock().unwrap();
                (
                    ler_config(&conn, CHAVE_PROJETO_URL)?
                        .ok_or("Falha ao salvar a configuração do .env.")?,
                    ler_config(&conn, CHAVE_ANON_KEY)?
                        .ok_or("Falha ao salvar a configuração do .env.")?,
                    ler_config(&conn, CHAVE_EMAIL)?.unwrap_or_default(),
                    ler_config(&conn, CHAVE_REFRESH_TOKEN)?.unwrap_or_default(),
                )
            } else {
                return Err(
                    "A conta da nuvem não está conectada. Conecte-a na tela de Sincronização."
                        .to_string(),
                );
            }
        }
    };
    let cfg = ConfiguracaoConexao {
        projeto_url: url,
        anon_key: anon,
        email,
        senha: String::new(),
    };

    // 2) Renova o access token (a rotação devolve um novo refresh token).
    let (access, refresh) = renovar_gotrue(http, &cfg, &refresh).await?;
    {
        let conn = db.lock().unwrap();
        salvar_config(&conn, CHAVE_ACCESS_TOKEN, &access)?;
        salvar_config(&conn, CHAVE_REFRESH_TOKEN, &refresh)?;
    }
    let uid = uid_do_token(http, &cfg, &access).await?;

    // 3) Push: registros locais alterados desde o último envio (ou todos,
    //    na primeira vez — marcador vazio), em DUAS fases:
    //    A) criações/edições, na ordem das FKs (companies → employees →
    //       periods) — a empresa precisa existir na nuvem antes do funcionário;
    //    B) remoções (soft delete), na ordem INVERSA (periods → employees →
    //       companies) — a nuvem bloqueia excluir empresa com funcionários
    //       ativos (mesma regra do app).
    //    Cada envio devolve o `updated_at` que o SERVIDOR gravou — esse valor
    //    carimba a linha local, então o relógio da máquina não interfere na
    //    detecção de "já enviei". Um registro recusado (ex.: CNPJ/CPF
    //    duplicado entre máquinas) vira aviso e NÃO aborta o restante.
    let push_ate = {
        let conn = db.lock().unwrap();
        ler_config(&conn, CHAVE_PUSH_ATE)?.unwrap_or_default()
    };
    let mut pendentes: Vec<(String, String)> = Vec::new(); // (tabela, id)
    let mut apagados: Vec<(String, String)> = Vec::new();
    for tabela in TABELAS {
        for id in linhas_locais_alteradas(db, tabela, &push_ate)? {
            if registro_esta_apagado(db, tabela, &id)? {
                apagados.push((tabela.to_string(), id));
            } else {
                pendentes.push((tabela.to_string(), id));
            }
        }
    }
    // Remoções na ordem filho → pai.
    let mut apagados_ordenados: Vec<(String, String)> = Vec::new();
    for tabela in ["users", "employee_leave_periods", "employees", "companies"] {
        apagados_ordenados.extend(
            apagados
                .iter()
                .filter(|(tabela_atual, _)| tabela_atual == tabela)
                .cloned(),
        );
    }

    let mut max_enviado_ate = String::new();
    empurrar_lote(
        db,
        http,
        &cfg,
        &access,
        pendentes,
        &mut resumo,
        &mut max_enviado_ate,
    )
    .await?;
    empurrar_lote(
        db,
        http,
        &cfg,
        &access,
        apagados_ordenados,
        &mut resumo,
        &mut max_enviado_ate,
    )
    .await?;

    // 4) Pull: deltas da nuvem desde a última puxada. Um erro isolado ao
    //    aplicar um registro vira aviso (não aborta o restante).
    let pull_ate = {
        let conn = db.lock().unwrap();
        ler_config(&conn, CHAVE_PULL_ATE)?.unwrap_or_default()
    };
    let mut novo_pull_ate = pull_ate.clone();
    for tabela in TABELAS {
        for remoto in puxar_deltas(http, &cfg, &access, &uid, tabela, &pull_ate).await? {
            match aplicar_registro_remoto(db, tabela, &remoto, &mut resumo.avisos) {
                Ok(aplicado) => {
                    if aplicado {
                        resumo.recebidos += 1;
                    }
                    if let Some(ts) = remoto.get("updated_at").and_then(|v| v.as_str()) {
                        if let Some(normalizado) = normalizar_ts_servidor(ts) {
                            if normalizado > novo_pull_ate {
                                novo_pull_ate = normalizado;
                            }
                        }
                    }
                }
                Err(motivo) => resumo.avisos.push(motivo),
            }
        }
    }

    // 5) Marcadores + horário da última sincronização.
    //    `push_ate` anda junto com o relógio do SERVIDOR (máximo entre o que
    //    enviamos e o que recebemos) — nada do que acabou de ser enviado ou
    //    recebido é reenviado no próximo ciclo.
    let agora = {
        let conn = db.lock().unwrap();
        agora_sqlite(&conn)?
    };
    let ate_onde_andou = if max_enviado_ate > novo_pull_ate {
        max_enviado_ate
    } else {
        novo_pull_ate.clone()
    };
    let conn = db.lock().unwrap();
    salvar_config(&conn, CHAVE_PULL_ATE, &novo_pull_ate)?;
    salvar_config(&conn, CHAVE_PUSH_ATE, &ate_onde_andou)?;
    salvar_config(&conn, CHAVE_ULTIMA_SYNC, &agora)?;

    Ok(resumo)
}

// ═══════════════════ Push ═══════════════════════════════════════════════════

/// Ids (UUID) dos registros locais alterados desde o último envio.
/// Inclui os apagados (soft delete também altera `updated_at`).
fn linhas_locais_alteradas(
    db: &Mutex<Connection>,
    tabela: &str,
    desde: &str,
) -> Result<Vec<String>, String> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT CAST(id AS TEXT) FROM {tabela} WHERE updated_at > ?1 ORDER BY updated_at"
        ))
        .map_err(|err| format!("Falha ao preparar o envio de {tabela}: {err}"))?;

    let ids = stmt
        .query_map([desde], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao consultar alterações de {tabela}: {err}"))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|err| format!("Falha ao ler alterações de {tabela}: {err}"))?;
    Ok(ids)
}

/// Monta o JSON de um registro local (sem `updated_at`/`owner_id` — a nuvem
/// administra esses campos) e faz o upsert via PostgREST.
///
/// Pede `return=representation` para receber o registro já gravado e devolve
/// o `updated_at` do SERVIDOR (normalizado) — usado para carimbar a linha
/// local e evitar reenvios.
async fn empurrar_registro(
    db: &Mutex<Connection>,
    http: &reqwest::Client,
    cfg: &ConfiguracaoConexao,
    access: &str,
    tabela: &str,
    id: &str,
) -> Result<Option<String>, String> {
    let payload = {
        let conn = db.lock().unwrap();
        payload_do_registro(&conn, tabela, id)?
    };

    let corpo_envio = serde_json::to_string(&payload)
        .map_err(|err| format!("Falha ao montar o registro {tabela}/{id}: {err}"))?;

    let resposta = http
        .post(format!(
            "{}/rest/v1/{tabela}",
            cfg.projeto_url.trim_end_matches('/')
        ))
        .header("apikey", cfg.anon_key.trim())
        .header("Authorization", format!("Bearer {access}"))
        .header("Content-Type", "application/json")
        .header(
            "Prefer",
            "resolution=merge-duplicates,return=representation",
        )
        .body(corpo_envio)
        .send()
        .await
        .map_err(|err| format!("Falha ao enviar {tabela}/{id} para a nuvem: {err}"))?;

    if !resposta.status().is_success() {
        let corpo = resposta.text().await.unwrap_or_default();
        return Err(format!("A nuvem recusou o registro {tabela}/{id}: {corpo}"));
    }

    // O servidor devolve o registro gravado (array) — aproveita o updated_at
    // dele para carimbar a linha local.
    let corpo = resposta
        .text()
        .await
        .map_err(|err| format!("Falha ao ler a resposta do envio de {tabela}/{id}: {err}"))?;
    let ts_servidor: Option<String> = serde_json::from_str::<serde_json::Value>(&corpo)
        .ok()
        .and_then(|valor| valor.as_array().cloned())
        .and_then(|linhas| linhas.into_iter().next())
        .and_then(|linha| {
            linha
                .get("updated_at")
                .and_then(|v| v.as_str())
                .and_then(normalizar_ts_servidor)
        });

    if let Some(ts) = &ts_servidor {
        // Carimba a linha com o tempo do servidor (conteúdo já é idêntico ao
        // que enviamos) — o registro não volta a ser "alterado localmente".
        let conn = db.lock().unwrap();
        conn.execute(
            &format!("UPDATE {tabela} SET updated_at = ?1 WHERE id = ?2"),
            params![ts, id],
        )
        .map_err(|err| format!("Falha ao carimbar o registro enviado {tabela}/{id}: {err}"))?;
    }

    Ok(ts_servidor)
}

/// Lê uma linha local e a converte no JSON esperado pela tabela da nuvem.
fn payload_do_registro(
    conn: &Connection,
    tabela: &str,
    id: &str,
) -> Result<serde_json::Value, String> {
    // Usuário tem payload próprio (acessos embutidos) — fora do fluxo comum.
    if tabela == "users" {
        return payload_do_usuario(conn, id);
    }

    let valor = match tabela {
        "companies" => conn.query_row(
            "SELECT cnpj, razao_social, nome_fantasia, situacao, municipio, uf,
                        telefone, email, deleted_at
                   FROM companies WHERE id = ?1",
            [id],
            |l| {
                Ok(serde_json::json!({
                    "id": id,
                    "cnpj": l.get::<_, String>(0)?,
                    "razao_social": l.get::<_, String>(1)?,
                    "nome_fantasia": l.get::<_, Option<String>>(2)?,
                    "situacao": l.get::<_, Option<String>>(3)?,
                    "municipio": l.get::<_, Option<String>>(4)?,
                    "uf": l.get::<_, Option<String>>(5)?,
                    "telefone": l.get::<_, Option<String>>(6)?,
                    "email": l.get::<_, Option<String>>(7)?,
                    "deleted_at": l.get::<_, Option<String>>(8)?,
                }))
            },
        ),
        "employees" => conn.query_row(
            "SELECT nome, cpf, data_admissao, empresa_id, deleted_at
                   FROM employees WHERE id = ?1",
            [id],
            |l| {
                Ok(serde_json::json!({
                    "id": id,
                    "nome": l.get::<_, String>(0)?,
                    "cpf": l.get::<_, Option<String>>(1)?,
                    "data_admissao": l.get::<_, Option<String>>(2)?,
                    "empresa_id": l.get::<_, String>(3)?,
                    "deleted_at": l.get::<_, Option<String>>(4)?,
                }))
            },
        ),
        "employee_leave_periods" => conn.query_row(
            "SELECT employee_id, inicio, vencimento, regularizada,
                        regularizada_em, observacao, deleted_at
                   FROM employee_leave_periods WHERE id = ?1",
            [id],
            |l| {
                Ok(serde_json::json!({
                    "id": id,
                    "employee_id": l.get::<_, String>(0)?,
                    "inicio": l.get::<_, String>(1)?,
                    "vencimento": l.get::<_, String>(2)?,
                    "regularizada": l.get::<_, i64>(3)? != 0,
                    "regularizada_em": l.get::<_, Option<String>>(4)?,
                    "observacao": l.get::<_, Option<String>>(5)?,
                    "deleted_at": l.get::<_, Option<String>>(6)?,
                }))
            },
        ),
        _ => return Err(format!("Tabela não sincronizável: {tabela}")),
    };

    valor.map_err(|err| format!("Falha ao ler o registro {tabela}/{id}: {err}"))
}

/// Payload de um usuário local: dados + acessos por módulo embutidos
/// (`secoes`), já que a nuvem guarda os dois no mesmo registro.
fn payload_do_usuario(conn: &Connection, id: &str) -> Result<serde_json::Value, String> {
    let (username, display_name, password_hash, deleted_at) = conn
        .query_row(
            "SELECT username, display_name, password_hash, deleted_at
               FROM users WHERE id = ?1",
            [id],
            |l| {
                Ok((
                    l.get::<_, String>(0)?,
                    l.get::<_, String>(1)?,
                    l.get::<_, String>(2)?,
                    l.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .map_err(|err| format!("Falha ao ler o usuário {id}: {err}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT section_id FROM user_permissions
              WHERE user_id = ?1 ORDER BY section_id",
        )
        .map_err(|err| format!("Falha ao ler os acessos do usuário: {err}"))?;
    let secoes = stmt
        .query_map([id], |l| l.get::<_, String>(0))
        .map_err(|err| format!("Falha ao consultar os acessos: {err}"))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|err| format!("Falha ao ler os acessos: {err}"))?;

    Ok(serde_json::json!({
        "username": username,
        "display_name": display_name,
        "password_hash": password_hash,
        "secoes": secoes,
        "deleted_at": deleted_at,
    }))
}

/// O registro local está apagado (soft delete)? Determina a fase do push.
fn registro_esta_apagado(db: &Mutex<Connection>, tabela: &str, id: &str) -> Result<bool, String> {
    let conn = db.lock().unwrap();
    conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {tabela} WHERE id = ?1 AND deleted_at IS NOT NULL)"),
        [id],
        |linha| linha.get(0),
    )
    .map_err(|err| format!("Falha ao conferir o registro {tabela}/{id}: {err}"))
}

/// Envia uma lista de registros, tolerando recusas isoladas (viram avisos).
async fn empurrar_lote(
    db: &Mutex<Connection>,
    http: &reqwest::Client,
    cfg: &ConfiguracaoConexao,
    access: &str,
    itens: Vec<(String, String)>,
    resumo: &mut ResumoSync,
    max_enviado_ate: &mut String,
) -> Result<(), String> {
    for (tabela, id) in itens {
        match empurrar_registro(db, http, cfg, access, &tabela, &id).await {
            Ok(Some(ts)) => {
                if ts > *max_enviado_ate {
                    *max_enviado_ate = ts;
                }
                resumo.enviados += 1;
            }
            Ok(None) => resumo.enviados += 1,
            Err(motivo) => resumo
                .avisos
                .push(format!("Não foi possível enviar {tabela}/{id}: {motivo}")),
        }
    }
    Ok(())
}

// ═══════════════════ Pull ═══════════════════════════════════════════════════

/// Busca os registros alterados na nuvem (`updated_at > desde`), paginando.
async fn puxar_deltas(
    http: &reqwest::Client,
    cfg: &ConfiguracaoConexao,
    access: &str,
    uid: &str,
    tabela: &str,
    desde: &str,
) -> Result<Vec<serde_json::Value>, String> {
    // Espaço no marcador precisa de URL-encoding. No PRIMEIRO ciclo o marcador
    // é vazio → busca tudo (sem filtro de updated_at).
    let filtro_delta = if desde.trim().is_empty() {
        String::new()
    } else {
        format!("&updated_at=gt.{}", desde.replace(' ', "%20"))
    };
    let base = format!(
        "{}/rest/v1/{tabela}?select=*&owner_id=eq.{uid}{filtro_delta}&order=updated_at.asc",
        cfg.projeto_url.trim_end_matches('/')
    );

    const TAMANHO_PAGINA: usize = 500;
    let mut registros: Vec<serde_json::Value> = Vec::new();
    let mut inicio = 0usize;

    loop {
        let fim = inicio + TAMANHO_PAGINA - 1;
        let resposta = http
            .get(&base)
            .header("apikey", cfg.anon_key.trim())
            .header("Authorization", format!("Bearer {access}"))
            .header("Range", format!("{inicio}-{fim}"))
            .send()
            .await
            .map_err(|err| format!("Falha ao buscar {tabela} na nuvem: {err}"))?;

        if !resposta.status().is_success() {
            let corpo = resposta.text().await.unwrap_or_default();
            return Err(format!("A nuvem recusou a busca de {tabela}: {corpo}"));
        }

        let corpo = resposta
            .text()
            .await
            .map_err(|err| format!("Falha ao ler a resposta de {tabela}: {err}"))?;
        let pagina: Vec<serde_json::Value> = if corpo.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&corpo)
                .map_err(|err| format!("Resposta inválida de {tabela}: {err}"))?
        };

        let quantidade = pagina.len();
        registros.extend(pagina);

        // Continua enquanto a página vier COMPLETA (500): aí pode haver mais.
        // Se o servidor não informar o total (`content-range` com `*`) ou a
        // página vier menor, chegou ao fim.
        if quantidade == TAMANHO_PAGINA {
            inicio += TAMANHO_PAGINA;
        } else {
            break;
        }
    }

    Ok(registros)
}

/// Aplica um registro vindo da nuvem ao banco local (last-write-wins).
/// Devolve `true` se o registro foi aplicado (contabilizado como recebido).
fn aplicar_registro_remoto(
    db: &Mutex<Connection>,
    tabela: &str,
    remoto: &serde_json::Value,
    avisos: &mut Vec<String>,
) -> Result<bool, String> {
    let registro = match remoto.as_object() {
        Some(registro) => registro,
        None => return Ok(false),
    };

    // Usuários: identidade é o `username` (não há `id` no payload) e a
    // aplicação é própria (reativa/atualiza pelo nome + reescreve acessos).
    if tabela == "users" {
        let conn = db.lock().unwrap();
        return aplicar_usuario_remoto(&conn, registro, avisos);
    }

    let Some(id) = id_do_registro(registro) else {
        return Ok(false);
    };
    let Some(remoto_ts) = registro
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(normalizar_ts_servidor)
    else {
        return Ok(false); // sem timestamp utilizável — ignora
    };

    let conn = db.lock().unwrap();

    // Data de criação remota (normalizada) — usada quando o registro é novo.
    let remoto_criado = registro
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(normalizar_ts_servidor)
        .unwrap_or_else(|| remoto_ts.clone());
    let remoto_deletado = registro
        .get("deleted_at")
        .and_then(|v| v.as_str())
        .and_then(normalizar_ts_servidor);

    let local_ts: Option<String> = conn
        .query_row(
            &format!("SELECT updated_at FROM {tabela} WHERE id = ?1"),
            [&id],
            |l| l.get(0),
        )
        .optional()
        .map_err(|err| format!("Falha ao consultar {tabela}/{id}: {err}"))?;

    match tabela {
        "companies" => {
            if local_ts.is_none() {
                conn.execute(
                    "INSERT INTO companies
                        (id, cnpj, razao_social, nome_fantasia, situacao, municipio, uf,
                         telefone, email, created_at, updated_at, deleted_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        id,
                        campo_str(registro, "cnpj").unwrap_or_default(),
                        campo_str(registro, "razao_social").unwrap_or_default(),
                        campo_str(registro, "nome_fantasia"),
                        campo_str(registro, "situacao"),
                        campo_str(registro, "municipio"),
                        campo_str(registro, "uf"),
                        campo_str(registro, "telefone"),
                        campo_str(registro, "email"),
                        remoto_criado,
                        remoto_ts,
                        remoto_deletado,
                    ],
                )
                .map_err(|err| format!("Falha ao receber a empresa {id}: {err}"))?;
                Ok(true)
            } else {
                // Aplica só se a nuvem não for mais antiga que o local.
                let local_ts = local_ts.unwrap();
                if local_ts <= remoto_ts {
                    conn.execute(
                        "UPDATE companies
                            SET cnpj = ?1, razao_social = ?2, nome_fantasia = ?3,
                                situacao = ?4, municipio = ?5, uf = ?6, telefone = ?7,
                                email = ?8, deleted_at = ?9, updated_at = ?10
                          WHERE id = ?11",
                        params![
                            campo_str(registro, "cnpj").unwrap_or_default(),
                            campo_str(registro, "razao_social").unwrap_or_default(),
                            campo_str(registro, "nome_fantasia"),
                            campo_str(registro, "situacao"),
                            campo_str(registro, "municipio"),
                            campo_str(registro, "uf"),
                            campo_str(registro, "telefone"),
                            campo_str(registro, "email"),
                            remoto_deletado,
                            remoto_ts,
                            id,
                        ],
                    )
                    .map_err(|err| format!("Falha ao atualizar a empresa {id}: {err}"))?;
                    Ok(true)
                } else {
                    Ok(false) // local mais novo — mantém
                }
            }
        }
        "employees" => {
            let empresa_id = campo_str(registro, "empresa_id").unwrap_or_default();
            if local_ts.is_none() {
                // O vínculo (empresa) precisa existir localmente primeiro.
                let empresa_existe: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM companies WHERE id = ?1)",
                        [&empresa_id],
                        |l| l.get(0),
                    )
                    .unwrap_or(false);
                if !empresa_existe {
                    avisos.push(format!(
                        "Funcionário {id} aguarda a empresa de origem na próxima sincronização."
                    ));
                    return Ok(false);
                }
                conn.execute(
                    "INSERT INTO employees
                        (id, nome, cpf, data_admissao, empresa_id, created_at, updated_at, deleted_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        id,
                        campo_str(registro, "nome").unwrap_or_default(),
                        campo_str(registro, "cpf"),
                        campo_str(registro, "data_admissao"),
                        empresa_id,
                        remoto_criado,
                        remoto_ts,
                        remoto_deletado,
                    ],
                )
                .map_err(|err| format!("Falha ao receber o funcionário {id}: {err}"))?;
                Ok(true)
            } else {
                let local_ts = local_ts.unwrap();
                if local_ts <= remoto_ts {
                    conn.execute(
                        "UPDATE employees
                            SET nome = ?1, cpf = ?2, data_admissao = ?3,
                                deleted_at = ?4, updated_at = ?5
                          WHERE id = ?6",
                        params![
                            campo_str(registro, "nome").unwrap_or_default(),
                            campo_str(registro, "cpf"),
                            campo_str(registro, "data_admissao"),
                            remoto_deletado,
                            remoto_ts,
                            id,
                        ],
                    )
                    .map_err(|err| format!("Falha ao atualizar o funcionário {id}: {err}"))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
        "employee_leave_periods" => {
            let employee_id = campo_str(registro, "employee_id").unwrap_or_default();
            if local_ts.is_none() {
                let funcionario_existe: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM employees WHERE id = ?1)",
                        [&employee_id],
                        |l| l.get(0),
                    )
                    .unwrap_or(false);
                if !funcionario_existe {
                    avisos.push(format!(
                        "Período de férias {id} aguarda o funcionário de origem na próxima sincronização."
                    ));
                    return Ok(false);
                }
                conn.execute(
                    "INSERT INTO employee_leave_periods
                        (id, employee_id, inicio, vencimento, regularizada,
                         regularizada_em, observacao, created_at, updated_at, deleted_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        id,
                        employee_id,
                        campo_str(registro, "inicio").unwrap_or_default(),
                        campo_str(registro, "vencimento").unwrap_or_default(),
                        registro
                            .get("regularizada")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false) as i64,
                        campo_str(registro, "regularizada_em"),
                        campo_str(registro, "observacao"),
                        remoto_criado,
                        remoto_ts,
                        remoto_deletado,
                    ],
                )
                .map_err(|err| format!("Falha ao receber o período de férias {id}: {err}"))?;
                Ok(true)
            } else {
                let local_ts = local_ts.unwrap();
                if local_ts <= remoto_ts {
                    conn.execute(
                        "UPDATE employee_leave_periods
                            SET regularizada = ?1, regularizada_em = ?2, observacao = ?3,
                                deleted_at = ?4, updated_at = ?5
                          WHERE id = ?6",
                        params![
                            registro
                                .get("regularizada")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false) as i64,
                            campo_str(registro, "regularizada_em"),
                            campo_str(registro, "observacao"),
                            remoto_deletado,
                            remoto_ts,
                            id,
                        ],
                    )
                    .map_err(|err| format!("Falha ao atualizar o período de férias {id}: {err}"))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
        _ => Err(format!("Tabela não sincronizável: {tabela}")),
    }
}

/// Aplica um usuário vindo da nuvem (identidade = `username`).
/// - Cria localmente se não existir (ou reativa um apagado, atualizando hash);
/// - Se a nuvem for mais nova, atualiza nome/hash/remoção e **reescreve os
///   acessos** (`user_permissions`) com a lista `secoes` recebida;
/// - Usuário removido ⇒ soft-delete local + encerra as sessões dele.
fn aplicar_usuario_remoto(
    conn: &Connection,
    registro: &serde_json::Map<String, serde_json::Value>,
    _avisos: &mut Vec<String>,
) -> Result<bool, String> {
    let Some(username) = campo_str(registro, "username") else {
        return Ok(false);
    };
    let Some(remoto_ts) = registro
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(normalizar_ts_servidor)
    else {
        return Ok(false);
    };

    // Superusuário nunca é removido pela nuvem.
    let removido = registro
        .get("deleted_at")
        .and_then(|v| v.as_str())
        .and_then(normalizar_ts_servidor);
    if username == crate::auth::SUPER_USUARIO && removido.is_some() {
        return Ok(false);
    }

    let local: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT id, updated_at FROM users WHERE username = ?1",
            [&username],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()
        .map_err(|err| format!("Falha ao consultar o usuário {username}: {err}"))?;

    let secoes_remotas: Vec<String> = registro
        .get("secoes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    match local {
        None => {
            // Cria o usuário localmente (o id local é gerado pelo banco).
            if removido.is_some() {
                return Ok(false); // nunca existiu localmente — nada a criar
            }
            conn.execute(
                "INSERT INTO users (username, display_name, password_hash)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    username,
                    campo_str(registro, "display_name").unwrap_or_default(),
                    campo_str(registro, "password_hash").unwrap_or_default(),
                ],
            )
            .map_err(|err| format!("Falha ao receber o usuário {username}: {err}"))?;
            let user_id = conn.last_insert_rowid();
            reescrever_acessos(conn, user_id, &secoes_remotas)?;
            Ok(true)
        }
        Some((user_id, local_ts)) => {
            let local_ts = local_ts.unwrap_or_default();
            if local_ts > remoto_ts {
                return Ok(false); // local mais novo — mantém
            }
            conn.execute(
                "UPDATE users
                    SET display_name = ?1, password_hash = ?2,
                        deleted_at = ?3, updated_at = ?4
                  WHERE id = ?5",
                rusqlite::params![
                    campo_str(registro, "display_name").unwrap_or_default(),
                    campo_str(registro, "password_hash").unwrap_or_default(),
                    removido,
                    remoto_ts,
                    user_id,
                ],
            )
            .map_err(|err| format!("Falha ao atualizar o usuário {username}: {err}"))?;

            if let Some(removido) = removido {
                // Usuário removido na nuvem: encerra as sessões locais dele.
                conn.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])
                    .map_err(|err| format!("Falha ao encerrar sessões de {username}: {err}"))?;
                let _ = removido;
            } else {
                reescrever_acessos(conn, user_id, &secoes_remotas)?;
            }
            Ok(true)
        }
    }
}

/// Substitui os acessos locais de um usuário pela lista remota.
fn reescrever_acessos(conn: &Connection, user_id: i64, secoes: &[String]) -> Result<(), String> {
    conn.execute("DELETE FROM user_permissions WHERE user_id = ?1", [user_id])
        .map_err(|err| format!("Falha ao limpar acessos locais: {err}"))?;
    for secao in secoes {
        conn.execute(
            "INSERT INTO user_permissions (user_id, section_id) VALUES (?1, ?2)",
            rusqlite::params![user_id, secao],
        )
        .map_err(|err| format!("Falha ao salvar o acesso \"{secao}\": {err}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    //! Testes das partes que não dependem de rede (timestamps e helpers).

    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    fn caminho_temporario() -> std::path::PathBuf {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("scd-sync-teste-{}-{n}.db", std::process::id()))
    }

    #[test]
    fn normaliza_timestamp_do_servidor() {
        assert_eq!(
            normalizar_ts_servidor("2025-05-05T12:34:56.789123+00:00").as_deref(),
            Some("2025-05-05 12:34:56")
        );
        assert_eq!(
            normalizar_ts_servidor("2025-05-05T12:34:56Z").as_deref(),
            Some("2025-05-05 12:34:56")
        );
        assert_eq!(
            normalizar_ts_servidor("2025-05-05T12:34:56+00:00").as_deref(),
            Some("2025-05-05 12:34:56")
        );
        assert_eq!(normalizar_ts_servidor("curto"), None);
    }

    #[test]
    fn deteccao_de_alteracoes_e_payload_funcionam_no_esquema_local() {
        let caminho = caminho_temporario();
        let conn = crate::db::open(&caminho).expect("abrir banco de teste falhou");
        let db = Mutex::new(conn);

        const EMPRESA_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const FUNC_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        const PERIODO_ID: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO companies (id, cnpj, razao_social) VALUES (?1, '12345678000199', 'Teste')",
                [EMPRESA_ID],
            )
            .expect("inserir empresa falhou");
            conn.execute(
                "INSERT INTO employees (id, nome, cpf, data_admissao, empresa_id)
                 VALUES (?1, 'Fulano', '52998224725', '2023-01-02', ?2)",
                rusqlite::params![FUNC_ID, EMPRESA_ID],
            )
            .expect("inserir funcionário falhou");
            conn.execute(
                "INSERT INTO employee_leave_periods (id, employee_id, inicio, vencimento)
                 VALUES (?1, ?2, '2023-01-02', '2025-01-02')",
                rusqlite::params![PERIODO_ID, FUNC_ID],
            )
            .expect("inserir período falhou");
        }

        // Tudo alterado desde o início (marcador vazio) é detectado.
        assert_eq!(
            linhas_locais_alteradas(&db, "companies", "").unwrap(),
            vec![EMPRESA_ID.to_string()]
        );
        assert!(!registro_esta_apagado(&db, "companies", EMPRESA_ID).unwrap());

        // O payload das 3 tabelas é montado sem erro (colunas corretas).
        {
            let conn = db.lock().unwrap();
            let empresa = payload_do_registro(&conn, "companies", EMPRESA_ID).unwrap();
            assert_eq!(empresa["razao_social"], "Teste");
            let funcionario = payload_do_registro(&conn, "employees", FUNC_ID).unwrap();
            assert_eq!(funcionario["empresa_id"], EMPRESA_ID);
            let periodo = payload_do_registro(&conn, "employee_leave_periods", PERIODO_ID).unwrap();
            assert_eq!(periodo["regularizada"], serde_json::Value::Bool(false));
        }

        // Após soft delete, o registro é detectado como apagado e continua
        // na lista de alterações (para o push enviar a remoção).
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE companies SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
                [EMPRESA_ID],
            )
            .unwrap();
        }
        assert!(registro_esta_apagado(&db, "companies", EMPRESA_ID).unwrap());
        assert_eq!(
            linhas_locais_alteradas(&db, "companies", "").unwrap(),
            vec![EMPRESA_ID.to_string()]
        );

        let _ = std::fs::remove_file(&caminho);
    }

    /// Prova de ponta a ponta do motor REAL contra a nuvem (requer o arquivo
    /// `.env` na raiz com credenciais válidas e acesso à internet). Ignorado
    /// por padrão — rode explicitamente com:
    ///   cargo test --lib e2e_sincroniza_com_nuvem_real -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "integração real com a nuvem (rede + .env)"]
    async fn e2e_sincroniza_com_nuvem_real() {
        let caminho = caminho_temporario();
        let conn = crate::db::open(&caminho).expect("abrir banco de teste falhou");
        let db = Mutex::new(conn);
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(8))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("criar cliente HTTP falhou");

        // 1) Banco vazio: deve conectar sozinho via .env e sincronizar (0/0).
        let resumo = sincronizar(&db, &http)
            .await
            .expect("primeiro ciclo falhou");
        let estado = estado(&db).expect("ler estado falhou");
        assert!(
            estado.conectado,
            "deveria ter conectado pela configuração do .env"
        );
        println!(
            "[e2e] conectado como {:?} | enviados={} recebidos={} avisos={:?}",
            estado.email, resumo.enviados, resumo.recebidos, resumo.avisos
        );

        // 2) Cria uma empresa E um usuário local (com acessos) e sincroniza:
        //    ambos precisam ir para a nuvem (usuário com `secoes` embutidas).
        let id_empresa = uuid::Uuid::new_v4().to_string();
        let cnpj = "33333333000199";
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO companies (id, cnpj, razao_social) VALUES (?1, ?2, 'SCD Teste E2E')",
                rusqlite::params![id_empresa, cnpj],
            )
            .expect("inserir empresa local falhou");
            conn.execute(
                "INSERT INTO users (username, display_name, password_hash)
                 VALUES ('usuaria-e2e', 'Usuária E2E', 'hash-e2e')",
                [],
            )
            .expect("inserir usuária local falhou");
            let user_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO user_permissions (user_id, section_id) VALUES (?1, 'cadastro-empresa')",
                [user_id],
            )
            .expect("inserir acesso da usuária falhou");
        }
        let resumo2 = sincronizar(&db, &http).await.expect("segundo ciclo falhou");
        println!(
            "[e2e] push empresa+usuário -> enviados={} recebidos={} avisos={:?}",
            resumo2.enviados, resumo2.recebidos, resumo2.avisos
        );
        assert!(
            resumo2.enviados >= 2,
            "empresa e usuário deveriam ter sido enviados"
        );
        assert!(
            resumo2.avisos.is_empty(),
            "não deveria haver avisos: {:?}",
            resumo2.avisos
        );

        // 3) Soft-delete local (empresa + usuário): a remoção precisa chegar à
        //    nuvem no próximo ciclo.
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE companies SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
                [&id_empresa],
            )
            .expect("soft delete local falhou");
            conn.execute(
                "UPDATE users SET deleted_at = datetime('now'), updated_at = datetime('now')
                  WHERE username = 'usuaria-e2e'",
                [],
            )
            .expect("soft delete da usuária falhou");
        }
        let resumo3 = sincronizar(&db, &http)
            .await
            .expect("terceiro ciclo falhou");
        println!(
            "[e2e] soft-delete -> enviados={} avisos={:?}",
            resumo3.enviados, resumo3.avisos
        );
        assert!(
            resumo3.enviados >= 2,
            "as remoções deveriam ter sido enviadas"
        );

        let _ = std::fs::remove_file(&caminho);
        println!("[e2e] OK — motor sincronizou com a nuvem real");
    }

    #[test]
    fn usuarios_payload_e_aplicacao_redonda() {
        // Origem: usuário local com acessos → payload com `secoes`.
        let caminho_origem = caminho_temporario();
        let conn_origem = crate::db::open(&caminho_origem).expect("abrir banco origem falhou");
        {
            conn_origem
                .execute(
                    "INSERT INTO users (username, display_name, password_hash)
                     VALUES ('maria', 'Maria Silva', 'hash-de-teste')",
                    [],
                )
                .expect("inserir usuária falhou");
            let id = conn_origem.last_insert_rowid();
            conn_origem
                .execute(
                    "INSERT INTO user_permissions (user_id, section_id) VALUES (?1, 'cadastro-empresa'), (?1, 'ferias-a-vencer')",
                    [id],
                )
                .expect("inserir acessos falhou");
        }
        let mut payload = payload_do_usuario(
            &conn_origem,
            &conn_origem
                .query_row("SELECT id FROM users WHERE username='maria'", [], |l| {
                    l.get::<_, i64>(0)
                })
                .expect("id da usuária")
                .to_string(),
        )
        .expect("payload da usuária falhou");
        assert_eq!(payload["username"], "maria");
        assert_eq!(payload["secoes"].as_array().map(Vec::len), Some(2));

        // O servidor acrescenta o updated_at (ausente no payload local).
        payload["updated_at"] = serde_json::Value::String("2026-01-01 10:00:00".to_string());

        // Destino: aplica o registro remoto num banco vazio.
        let caminho_destino = caminho_temporario();
        let conn_destino = crate::db::open(&caminho_destino).expect("abrir banco destino falhou");
        let mut avisos = Vec::new();
        let aplicado = aplicar_usuario_remoto(
            &conn_destino,
            payload.as_object().expect("objeto"),
            &mut avisos,
        )
        .expect("aplicar usuária falhou");
        assert!(aplicado);

        let (nome, hash): (String, String) = conn_destino
            .query_row(
                "SELECT display_name, password_hash FROM users WHERE username = 'maria'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .expect("usuária não chegou");
        assert_eq!(nome, "Maria Silva");
        assert_eq!(hash, "hash-de-teste");
        let total_acessos: i64 = conn_destino
            .query_row(
                "SELECT COUNT(*) FROM user_permissions p JOIN users u ON u.id = p.user_id
                  WHERE u.username = 'maria'",
                [],
                |l| l.get(0),
            )
            .expect("contar acessos");
        assert_eq!(total_acessos, 2, "acessos devem ter sido reescritos");

        let _ = std::fs::remove_file(&caminho_origem);
        let _ = std::fs::remove_file(&caminho_destino);
    }
}
