//! Comandos Tauri do controle de férias (camada fina).
//! As regras ficam em `ferias.rs`; aqui só traduzimos as chamadas do Vue.
//!
//! As notificações "nativas" NÃO usam o plugin do Tauri: abrimos uma
//! SEGUNDA JANELA própria (sem borda, sempre no topo, canto superior
//! direito) que permanece na tela até o usuário clicar em um botão.

use serde::Serialize;
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::{ferias, AppState};

/// Nome da janela de notificação (secundária).
const JANELA_NOTIFICACAO: &str = "notificacao";

/// Comando `listar_ferias_vencidas` — períodos vencidos sem regularizar.
#[tauri::command]
pub fn listar_ferias_vencidas(
    state: State<'_, AppState>,
) -> Result<Vec<ferias::FeriasVencida>, String> {
    let db = state.db.lock().unwrap();
    ferias::listar_vencidas(&db)
}

/// Comando `listar_ferias_a_vencer` — direitos adquiridos dentro do prazo.
#[tauri::command]
pub fn listar_ferias_a_vencer(
    state: State<'_, AppState>,
) -> Result<Vec<ferias::FeriasAVencer>, String> {
    let db = state.db.lock().unwrap();
    let alerta = ferias::obter_alerta_dias(&db)?;
    ferias::listar_a_vencer(&db, alerta)
}

/// Comando `regularizar_ferias` — registra que o funcionário já gozou/quitou
/// o período vencido (encerra a pendência).
#[tauri::command]
pub fn regularizar_ferias(
    state: State<'_, AppState>,
    id: String,
    observacao: Option<String>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    ferias::regularizar(&db, &id, observacao)
}

/// Comando `obter_alerta_ferias` — dias de antecedência do alerta.
#[tauri::command]
pub fn obter_alerta_ferias(state: State<'_, AppState>) -> Result<i64, String> {
    let db = state.db.lock().unwrap();
    ferias::obter_alerta_dias(&db)
}

/// Comando `definir_alerta_ferias` — configura os dias de antecedência.
#[tauri::command]
pub fn definir_alerta_ferias(state: State<'_, AppState>, dias: i64) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    ferias::definir_alerta_dias(&db, dias)
}

/// Comando `listar_notificacoes` — notificações internas (sino do app).
#[tauri::command]
pub fn listar_notificacoes(state: State<'_, AppState>) -> Result<Vec<ferias::Notificacao>, String> {
    let db = state.db.lock().unwrap();
    ferias::listar_notificacoes(&db)
}

// ═══════════════════ Janela própria de notificação ═════════════════════════

/// Chaves de memória (app_settings) usadas pela janela de notificações.
const CHAVE_SONECA: &str = "notificacoes.os.soneca_ate";
const CHAVE_ULTIMA_VEZ: &str = "notificacoes.os.ultima_vez";
/// Vencidas reabrem após 10 min (se a janela foi fechada sem ação).
const REENVIO_VENCIDA_SEG: u64 = 10 * 60;
/// Alertas "a vencer": reenvio a cada 6 horas.
const REENVIO_ALERTA_SEG: u64 = 6 * 3600;

/// A primeira checagem de cada execução SEMPRE abre a janela (se houver
/// pendência e não estiver com "lembrar mais tarde" ativo). Evita que uma
/// memória antiga impeça a notificação de aparecer ao abrir o app.
static PRIMEIRA_CHECAGEM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Comando `verificar_notificacoes_os` — se houver pendência devida, abre a
/// JANELA DE NOTIFICAÇÃO (janela própria do app, sem o plugin do Tauri).
/// A janela permanece visível até o usuário clicar em um dos botões.
#[tauri::command]
pub fn verificar_notificacoes_os(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    use std::sync::atomic::Ordering;

    let db = state.db.lock().unwrap();

    // "Lembrar mais tarde" soneca até o fim do dia.
    let hoje: String = db
        .query_row("SELECT date('now','localtime')", [], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao ler a data atual: {err}"))?;
    if ferias::obter_config(&db, CHAVE_SONECA)? == Some(hoje.clone()) {
        return Ok(0);
    }

    // Já existe uma janela de notificação aberta? Mantém a atual.
    if app.get_webview_window(JANELA_NOTIFICACAO).is_some() {
        return Ok(0);
    }

    let primeira_chamada = !PRIMEIRA_CHECAGEM.swap(true, Ordering::SeqCst);

    let agora_seg = agora_em_segundos();
    let ultima_vez = ler_mapa_ultima_vez(&db)?;
    let atuais = ferias::listar_notificacoes(&db)?;

    // Regra central: NADA a notificar ⇒ nenhuma janela. Se sobrou alguma
    // janela antiga (de uma versão anterior), fechamos para não "travar".
    if atuais.is_empty() {
        eprintln!("[notificacao] nada a notificar — janela não será aberta.");
        drop(db);
        let _ = fechar_janela(app);
        return Ok(0);
    }

    // Intervalo de reenvio por tipo.
    let intervalo = |tipo: &str| -> u64 {
        if tipo == "vencida" {
            REENVIO_VENCIDA_SEG
        } else {
            REENVIO_ALERTA_SEG
        }
    };

    // Primeira pendência: na primeira chamada da execução abre SEMPRE;
    // depois, respeita o intervalo de reenvio.
    let proxima = atuais.into_iter().find(|n| {
        primeira_chamada
            || match ultima_vez.get(&n.id) {
                Some(ultima) => agora_seg.saturating_sub(*ultima) >= intervalo(&n.tipo),
                None => true,
            }
    });

    let Some(notificacao) = proxima else {
        return Ok(0);
    };

    // Marca como mostrada agora (evita repetir enquanto a janela está ativa).
    let mut nova_memoria = ultima_vez;
    nova_memoria.insert(notificacao.id.clone(), agora_seg);
    salvar_mapa_ultima_vez(&db, &nova_memoria)?;

    // Guarda a pendência que a página noti.html vai exibir (sem dados na URL).
    let secao = if notificacao.tipo == "vencida" {
        "cadastro-ferias-vencidas"
    } else {
        "ferias-a-vencer"
    };
    let pendencia = ferias::PendenciaNotificacao {
        id: notificacao.id.clone(),
        tipo: notificacao.tipo.clone(),
        titulo: notificacao.titulo.clone(),
        mensagem: notificacao.mensagem.clone(),
        secao: secao.to_string(),
    };
    *state.pendencia_notificacao.lock().unwrap() = Some(pendencia);
    drop(db); // libera o banco ANTES de abrir a janela

    // Abertura da janela numa THREAD própria, fora do caminho do comando:
    // criar uma segunda janela (WebView2) de dentro do comando segura o
    // worker do runtime enquanto o webview inicializa/carrega — com poucos
    // núcleos, isso deixa as listagens do app (listar_empresas e afins)
    // PRESAS na fila: o dashboard fica "carregando=true" para sempre, sem
    // erro algum. Com a thread, o comando devolve na hora e a janela abre
    // logo em seguida, sem travar as outras chamadas.
    {
        let app_para_janela = app.clone();
        std::thread::spawn(move || {
            // Pequeno atraso: garante que o comando já retornou e o app
            // terminou de processar a fila de comandos pendentes.
            std::thread::sleep(std::time::Duration::from_millis(300));
            if let Err(err) = abrir_janela_notificacao(&app_para_janela) {
                // Log visível no console do `tauri dev`.
                eprintln!("[notificacao] falha ao abrir a janela (thread): {err}");
            }
        });
    }
    Ok(1)
}

/// Abre a janela de notificação (lê a pendência guardada no estado).
fn abrir_janela_notificacao(app: &tauri::AppHandle) -> Result<usize, String> {
    let pendencia = {
        let estado = app.state::<crate::AppState>();
        let guard = estado.pendencia_notificacao.lock().unwrap();
        guard.clone()
    };
    let Some(pendencia) = pendencia else {
        // Segunda verificação: sem pendência registrada, NUNCA abre a janela.
        eprintln!("[notificacao] abortando: nenhuma pendência registrada para exibir.");
        return Ok(0);
    };

    eprintln!(
        "[notificacao] abrindo janela de notificação (id={}, tipo={})",
        pendencia.id, pendencia.tipo
    );

    // Posição: canto superior direito do monitor principal (estilo MSN).
    let (x, y) = match app
        .get_webview_window("main")
        .and_then(|w| w.current_monitor().ok().flatten())
    {
        Some(monitor) => {
            let largura = monitor.size().width as f64;
            ((largura - 400.0).max(8.0), 12.0)
        }
        None => (16.0, 12.0),
    };

    // Sem parâmetros na URL: a página busca o conteúdo pelo comando
    // obter_pendencia_notificacao (mais confiável que query string).
    {
        let estado = app.state::<crate::AppState>();
        let mut pronta = estado.janela_notificacao_pronta.lock().unwrap();
        *pronta = false;
    }

    let janela =
        WebviewWindowBuilder::new(app, JANELA_NOTIFICACAO, WebviewUrl::App("noti.html".into()))
            .inner_size(388.0, 240.0)
            .position(x, y)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .title("SCD — Notificação")
            .build()
            .map_err(|err| format!("Falha ao abrir a janela de notificação: {err}"))?;

    let _ = janela.set_focus();

    // Vigia de segurança: se a página não se declarar pronta em 8 s
    // (ex.: conteúdo não carregou), fecha a janela para não travar a tela.
    {
        let app_para_vigia = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(8));
            let pronto = app_para_vigia
                .state::<crate::AppState>()
                .janela_notificacao_pronta
                .lock()
                .unwrap()
                .clone();
            if !pronto {
                eprintln!("[notificacao] janela não carregou em 8 s — fechando por segurança.");
                if let Some(janela) = app_para_vigia.get_webview_window(JANELA_NOTIFICACAO) {
                    let _ = janela.close();
                }
            }
        });
    }

    Ok(1)
}

/// Payload do evento enviado à janela principal ("ir-para-secao").
#[derive(Clone, Serialize)]
struct IrParaSecao {
    secao: String,
}

/// Comando `acao_notificacao` — chamado pelos BOTÕES da janela de
/// notificação. Encerra o ciclo (janela fecha) conforme a ação:
///   - lembrar      → soneca até amanhã;
///   - regularizar  → abre a janela principal na página Férias vencidas;
///   - ver          → abre a janela principal na página do alerta.
#[tauri::command]
pub fn acao_notificacao(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    acao: String,
    secao: String,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();

    if acao == "lembrar" {
        let hoje: String = db
            .query_row("SELECT date('now','localtime')", [], |linha| linha.get(0))
            .map_err(|err| format!("Falha ao ler a data atual: {err}"))?;
        ferias::salvar_config(&db, CHAVE_SONECA, &hoje)?;
        // Marca como já mostrada para não reabrir hoje.
        let mut memoria = ler_mapa_ultima_vez(&db)?;
        memoria.insert(id.clone(), agora_em_segundos());
        salvar_mapa_ultima_vez(&db, &memoria)?;
    } else {
        // regularizar/ver: leva o usuário à página correta no app principal.
        let secao = if secao.is_empty() {
            "cadastro-ferias-vencidas"
        } else {
            secao.as_str()
        };
        let _ = app.emit_to(
            "main",
            "ir-para-secao",
            IrParaSecao {
                secao: secao.to_string(),
            },
        );
    }

    fechar_janela(app.clone())?;

    // Garante que a janela principal apareça e ganhe foco.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

/// Comando `sonecar_notificacoes_os` — usado pelo botão "Lembrar mais tarde"
/// do popup interno: nenhuma janela de notificação reabre até o dia seguinte.
#[tauri::command]
pub fn sonecar_notificacoes_os(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    let hoje: String = db
        .query_row("SELECT date('now','localtime')", [], |linha| linha.get(0))
        .map_err(|err| format!("Falha ao ler a data atual: {err}"))?;
    ferias::salvar_config(&db, CHAVE_SONECA, &hoje)
}

/// Comando `fechar_janela_notificacao` — encerra a janela de notificação
/// (usado pela própria página em caso de segurança).
#[tauri::command]
pub fn fechar_janela_notificacao(app: tauri::AppHandle) -> Result<(), String> {
    fechar_janela(app)
}

fn fechar_janela(app: tauri::AppHandle) -> Result<(), String> {
    // Limpa a pendência em exibição e desarma o vigia.
    let estado = app.state::<crate::AppState>();
    *estado.pendencia_notificacao.lock().unwrap() = None;
    *estado.janela_notificacao_pronta.lock().unwrap() = false;

    if let Some(janela) = app.get_webview_window(JANELA_NOTIFICACAO) {
        janela
            .close()
            .map_err(|err| format!("Falha ao fechar a janela de notificação: {err}"))?;
    }
    Ok(())
}

// ═══════════════════ Helpers de memória/duração ═════════════════════════════

fn agora_em_segundos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lê o mapa "id → último envio (seg)" guardado em app_settings.
fn ler_mapa_ultima_vez(
    db: &rusqlite::Connection,
) -> Result<std::collections::HashMap<String, u64>, String> {
    Ok(ferias::obter_config(db, CHAVE_ULTIMA_VEZ)?
        .unwrap_or_default()
        .split(';')
        .filter(|s| !s.is_empty())
        .filter_map(|par| {
            let mut partes = par.splitn(2, '=');
            let chave = partes.next()?.to_string();
            let valor = partes.next()?.parse::<u64>().ok()?;
            Some((chave, valor))
        })
        .collect())
}

fn salvar_mapa_ultima_vez(
    db: &rusqlite::Connection,
    mapa: &std::collections::HashMap<String, u64>,
) -> Result<(), String> {
    let memoria = mapa
        .iter()
        .map(|(id, seg)| format!("{id}={seg}"))
        .collect::<Vec<_>>()
        .join(";");
    ferias::salvar_config(db, CHAVE_ULTIMA_VEZ, &memoria)
}

/// Comando `obter_pendencia_notificacao` — usado pela página noti.html para
/// ler o que exibir (o conteúdo NÃO vai pela URL, evitando janela em branco).
#[tauri::command]
pub fn obter_pendencia_notificacao(
    state: State<'_, AppState>,
) -> Option<ferias::PendenciaNotificacao> {
    state.pendencia_notificacao.lock().unwrap().clone()
}

/// Comando `notificacao_janela_pronta` — a página chamou depois de renderizar
/// com sucesso; desativa o vigia de segurança (que fecharia em 8 s).
#[tauri::command]
pub fn notificacao_janela_pronta(state: State<'_, AppState>) -> Result<(), String> {
    let mut pronta = state.janela_notificacao_pronta.lock().unwrap();
    *pronta = true;
    Ok(())
}
