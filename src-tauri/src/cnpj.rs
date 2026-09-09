//! Consulta de dados de CNPJ + regras de CNPJ.
//!
//! Fontes (na ordem, todas públicas e gratuitas):
//!   1. **Minha Receita** (https://minhareceita.org) — API aberta, sem chave
//!      e sem limite prático de consultas; dados abertos da Receita Federal.
//!   2. **BrasilAPI** (https://brasilapi.com.br) — fallback; limite de ~3
//!      consultas/segundo por IP (429), tratado com retry/backoff.
//!
//! As duas respondem com o mesmo formato, então o parser é único.
//! Para trocar/ordenar as fontes, edite a lista `PROVEDORES` abaixo.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Normaliza um CNPJ digitado (máscara, pontos, barras, espaços) para o
/// padrão do sistema: **apenas os 14 dígitos** — e já confere os dígitos
/// verificadores, evitando gastar consultas na API com CNPJ falso.
pub fn normalizar_cnpj(raw: &str) -> Result<String, String> {
    let digitos: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digitos.len() != 14 {
        return Err("CNPJ inválido — informe os 14 dígitos.".to_string());
    }
    if !validar_cnpj(&digitos) {
        return Err("CNPJ inválido — os dígitos verificadores não conferem.".to_string());
    }
    Ok(digitos)
}

/// Valida os **dígitos verificadores** de um CNPJ com 14 dígitos
/// (algoritmo módulo 11, mesmo usado pela Receita Federal).
/// Isso roda ANTES de qualquer consulta externa.
pub fn validar_cnpj(cnpj: &str) -> bool {
    let digitos: Vec<u8> = cnpj
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| d as u8))
        .collect();
    if digitos.len() != 14 {
        return false;
    }

    // Pesos dos dois dígitos verificadores (da direita para a esquerda).
    const PESOS_DV1: [u8; 12] = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    const PESOS_DV2: [u8; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];

    let digito = |pesos: &[u8]| -> u8 {
        let soma: u32 = pesos
            .iter()
            .zip(digitos.iter())
            .map(|(peso, digito)| (*peso as u32) * (*digito as u32))
            .sum();
        let resto = soma % 11;
        if resto < 2 {
            0
        } else {
            11 - resto as u8
        }
    };

    let dv1 = digito(&PESOS_DV1);
    if dv1 != digitos[12] {
        return false;
    }
    let dv2 = digito(&PESOS_DV2);
    dv2 == digitos[13]
}

/// Dados de uma empresa vindos da API (exibidos antes de salvar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DadosEmpresa {
    pub cnpj: String,
    pub razao_social: String,
    pub nome_fantasia: Option<String>,
    pub situacao: Option<String>,
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
}

/// Um provedor de dados de CNPJ.
struct Provedor {
    nome: &'static str,
    /// Base da URL; a consulta é feita em `{base}/{cnpj}`.
    base: &'static str,
}

/// ⭐ Ordem de tentativa dos provedores (o primeiro que responder é usado).
/// Coloque primeiro a fonte com mais folga de consultas.
const PROVEDORES: &[Provedor] = &[
    Provedor {
        nome: "Minha Receita",
        base: "https://minhareceita.org",
    },
    Provedor {
        nome: "BrasilAPI",
        base: "https://brasilapi.com.br/api/cnpj/v1",
    },
];

/// Quantas tentativas fazemos por provedor ao receber `429` (limite).
const MAX_TENTATIVAS: usize = 4;
/// Espera inicial do retry; dobra a cada tentativa (1s → 2s → 4s).
const RETRY_INICIAL: Duration = Duration::from_secs(1);
/// User-Agent com "cara de navegador": alguns provedores (ex.: Minha Receita,
/// atrás de Cloudflare) bloqueiam/desafiam clientes com UA de biblioteca.
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

/// Busca os dados públicos de um CNPJ, tentando cada provedor de `PROVEDORES`.
/// `cnpj` deve estar normalizado (14 dígitos) — use `normalizar_cnpj` antes.
pub async fn buscar_por_cnpj(http: &reqwest::Client, cnpj: &str) -> Result<DadosEmpresa, String> {
    let mut ultimo_erro: Option<String> = None;

    for provedor in PROVEDORES {
        match consultar_com_retry(http, provedor, cnpj).await {
            Ok(dados) => return Ok(dados),
            // CNPJ inexistente vale para qualquer fonte — não adianta tentar a próxima.
            Err(FalhaConsulta::NaoEncontrado) => {
                return Err("CNPJ não encontrado nas fontes públicas consultadas.".to_string())
            }
            Err(FalhaConsulta::Outra(mensagem)) => {
                // Falha temporária (rede, 5xx, limite): anota e tenta o próximo provedor.
                ultimo_erro = Some(format!("{}: {mensagem}", provedor.nome));
            }
            // `Limite` só existe dentro do retry — nunca chega aqui.
            Err(FalhaConsulta::Limite { .. }) => unreachable!("consultar_com_retry não devolve Limite"),
        }
    }

    Err(ultimo_erro
        .unwrap_or_else(|| "Não foi possível consultar o CNPJ. Tente novamente.".to_string()))
}

/// Consulta um provedor, repetindo quando ele responder `429` (limite).
async fn consultar_com_retry(
    http: &reqwest::Client,
    provedor: &Provedor,
    cnpj: &str,
) -> Result<DadosEmpresa, FalhaConsulta> {
    let mut espera = RETRY_INICIAL;

    for _ in 1..=MAX_TENTATIVAS {
        match consultar_provedor(http, provedor.base, cnpj).await {
            Ok(dados) => return Ok(dados),
            Err(FalhaConsulta::Limite { espera_sugerida }) => {
                // 429: aguarda (o tempo indicado pela API, quando houver) e tenta de novo.
                let aguardar = espera_sugerida.unwrap_or(espera);
                tokio::time::sleep(aguardar).await;
                espera = espera.saturating_mul(2);
            }
            Err(outra) => return Err(outra),
        }
    }

    Err(FalhaConsulta::Outra(
        "limite de consultas atingido mesmo após novas tentativas.".to_string(),
    ))
}

/// Falha de uma consulta individual.
enum FalhaConsulta {
    /// HTTP 429 — limite de consultas; `espera_sugerida` vem da API quando informada.
    Limite { espera_sugerida: Option<Duration> },
    /// CNPJ não existe (404) — situação definitiva.
    NaoEncontrado,
    /// Qualquer outro erro (rede, 5xx...) — pode valer a pena tentar outra fonte.
    Outra(String),
}

/// Executa UMA consulta num provedor (sem retry — quem repete é `consultar_com_retry`).
async fn consultar_provedor(
    http: &reqwest::Client,
    base: &str,
    cnpj: &str,
) -> Result<DadosEmpresa, FalhaConsulta> {
    let url = format!("{base}/{cnpj}");

    let resposta = http
        .get(&url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|err| FalhaConsulta::Outra(format!("falha de conexão: {err}")))?;

    match resposta.status() {
        reqwest::StatusCode::OK => {
            // reqwest 0.13 não expõe `.json()` por padrão — decodificamos
            // manualmente a partir dos bytes da resposta.
            let corpo = resposta
                .bytes()
                .await
                .map_err(|err| FalhaConsulta::Outra(format!("falha ao ler a resposta: {err}")))?;
            let json: serde_json::Value = serde_json::from_slice(&corpo)
                .map_err(|err| FalhaConsulta::Outra(format!("resposta inválida: {err}")))?;
            Ok(extrair_dados(json, cnpj))
        }
        reqwest::StatusCode::NOT_FOUND => Err(FalhaConsulta::NaoEncontrado),
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            // Tenta descobrir por quanto tempo a API pede para esperar
            // (cabeçalho Retry-After ou mensagem "retry in N seconds").
            // Obs.: ler o cabeçalho ANTES de consumir o corpo (o corpo move a resposta).
            let espera = cabecalho_retry_after(&resposta);
            let corpo = resposta.text().await.unwrap_or_default();
            let espera = espera.or_else(|| segundos_do_retry(&corpo));
            Err(FalhaConsulta::Limite { espera_sugerida: espera })
        }
        _ => {
            // Erros das APIs trazem uma mensagem no corpo (JSON "message").
            let corpo = resposta.text().await.unwrap_or_default();
            let mensagem = serde_json::from_str::<serde_json::Value>(&corpo)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_string))
                .unwrap_or_else(|| {
                    let corpo = corpo.trim();
                    if corpo.is_empty() {
                        "erro do servidor".to_string()
                    } else {
                        corpo.to_string()
                    }
                });
            Err(FalhaConsulta::Outra(mensagem))
        }
    }
}

/// Lê o cabeçalho `Retry-After` (em segundos), quando presente.
fn cabecalho_retry_after(resposta: &reqwest::Response) -> Option<Duration> {
    let segundos = resposta
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()?;
    Some(Duration::from_secs(segundos))
}

/// Extrai "N" da mensagem do tipo "Rate limit exceeded, retry in N seconds".
fn segundos_do_retry(corpo: &str) -> Option<Duration> {
    let indice = corpo.find("retry in")?;
    let resto = &corpo[indice + "retry in".len()..];
    let numero: String = resto.chars().take_while(|c| c.is_ascii_digit()).collect();
    numero.parse::<u64>().ok().map(Duration::from_secs)
}

/// Converte o JSON da API no nosso `DadosEmpresa`.
/// Campos vazios ou ausentes viram `None` (padronização dos dados).
fn extrair_dados(json: serde_json::Value, cnpj: &str) -> DadosEmpresa {
    // Procura um campo de texto pelos nomes informados (provedores/versões
    // podem nomear o mesmo dado de formas diferentes — acrescente alternativas
    // aqui quando necessário).
    let texto = |chaves: &[&str]| -> Option<String> {
        for chave in chaves {
            if let Some(valor) = json.get(chave) {
                let obtido = match valor {
                    serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                };
                if obtido.is_some() {
                    return obtido;
                }
            }
        }
        None
    };

    // Razão social vazia? Usa o nome fantasia; se ambos faltarem, um
    // rótulo com o próprio CNPJ (para não gravar linha sem nome).
    let razao_social = texto(&["razao_social"])
        .or_else(|| texto(&["nome_fantasia"]))
        .unwrap_or_else(|| format!("EMPRESA {cnpj}"));

    DadosEmpresa {
        cnpj: cnpj.to_string(),
        razao_social,
        nome_fantasia: texto(&["nome_fantasia"]),
        situacao: texto(&["descricao_situacao_cadastral"]),
        municipio: texto(&["municipio"]),
        uf: texto(&["uf"]),
        telefone: texto(&["ddd_telefone_1", "ddd_telefone_2", "telefone"]),
        email: texto(&["email"]),
    }
}
