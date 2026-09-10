// Regras de EXIBIÇÃO e VALIDAÇÃO das férias — funções puras, sem Vue e sem
// Tauri, para poderem ser testadas automaticamente (`npm test`).
//
// A tela "Férias a vencer" e o formulário de agendamento importam daqui, de
// modo que o que está coberto pelos testes é exatamente o que o usuário vê.
//
// Obs.: o import abaixo usa a extensão `.ts` DE PROPÓSITO — o executor de
// testes do Node (`node --test`, sem nenhuma dependência extra) exige o
// caminho completo do arquivo. O Vite e o vue-tsc aceitam normalmente
// (`allowImportingTsExtensions` no tsconfig.json).
import { formatarDataBr } from "./data.ts";

/** Selo de situação do prazo (alerta automático). */
export interface SeloPrazo {
  texto: string;
  /** Está no vermelho/âmbar (vence hoje, amanhã ou dentro do alerta)? */
  alerta: boolean;
}

/**
 * Situação do prazo final do período, usada na coluna "Situação":
 * vence hoje / amanhã / em N dias (com ⚠ quando caiu na janela do alerta).
 */
export function seloDePrazo(diasRestantes: number, dentroAlerta: boolean): SeloPrazo {
  if (diasRestantes <= 0) return { texto: "vencem hoje", alerta: true };
  if (diasRestantes === 1) return { texto: "vencem amanhã", alerta: true };
  if (dentroAlerta) return { texto: `vencem em ${diasRestantes} dias ⚠`, alerta: true };
  return { texto: `faltam ${diasRestantes} dias`, alerta: false };
}

/**
 * Situação do alarme manual (férias agendadas): "aviso pendente" quando a data
 * escolhida já chegou, "avisa amanhã"/"avisa em N dia(s)" enquanto não chega.
 */
export function seloDoAlarme(alarmeDisparado: boolean, diasParaAlarme: number | null): string {
  if (alarmeDisparado) return "aviso pendente";
  if (diasParaAlarme === null) return "sem aviso";
  if (diasParaAlarme === 1) return "avisa amanhã";
  return `avisa em ${diasParaAlarme} dia(s)`;
}

/** Período mostrado no seletor do formulário de agendamento. */
export interface PeriodoAgendavel {
  inicio: string;
  vencimento: string;
  dias_restantes: number;
  alarme_em: string | null;
}

/**
 * Texto de uma opção do seletor de períodos:
 * `01/05/2024 → 01/05/2026 — faltam 400 dia(s) · agendado para 10/03/2026`.
 */
export function rotuloDoPeriodo(periodo: PeriodoAgendavel): string {
  const prazo = `${formatarDataBr(periodo.inicio)} → ${formatarDataBr(periodo.vencimento)}`;
  const faltam = periodo.dias_restantes <= 0 ? "vence hoje" : `faltam ${periodo.dias_restantes} dia(s)`;
  const agendado = periodo.alarme_em ? ` · agendado para ${formatarDataBr(periodo.alarme_em)}` : "";
  return `${prazo} — ${faltam}${agendado}`;
}

/**
 * "yyyy-mm-dd" é uma data real? (mesmo critério do backend: rejeita
 * 31/02, formato brasileiro e textos soltos.)
 */
export function dataIsoValida(iso: string): boolean {
  const partes = iso.split("-");
  if (partes.length !== 3) return false;
  const [textoAno, textoMes, textoDia] = partes;
  if (textoAno.length !== 4 || textoMes.length !== 2 || textoDia.length !== 2) return false;

  const ano = Number(textoAno);
  const mes = Number(textoMes);
  const dia = Number(textoDia);
  if (!Number.isInteger(ano) || !Number.isInteger(mes) || !Number.isInteger(dia)) return false;
  if (mes < 1 || mes > 12 || dia < 1) return false;

  // Dia 0 do mês seguinte = último dia do mês pedido (cuida de fevereiro/bissexto).
  return dia <= new Date(Date.UTC(ano, mes, 0)).getUTCDate();
}

/** Dados do formulário de agendamento (só o que a validação precisa). */
export interface Agendamento {
  funcionarioId: string | null;
  periodoId: string | null;
  alarmeEm: string;
  /** Data de hoje (yyyy-mm-dd) no fuso local. */
  hoje: string;
}

/**
 * Valida o formulário de agendamento antes de chamar o backend.
 * Devolve a mensagem de erro, ou `null` quando está tudo certo.
 *
 * Espelha as regras de `ferias::definir_alarme` (Rust): data real e nunca no
 * passado — assim o usuário recebe o aviso na hora, sem ida e volta ao banco.
 */
export function validarAgendamento(dados: Agendamento): string | null {
  if (!dados.funcionarioId) return "Escolha o funcionário.";
  if (!dados.periodoId) return "Escolha o período de férias.";
  if (!dados.alarmeEm) return "Informe a data do aviso.";
  if (!dataIsoValida(dados.alarmeEm)) {
    return "Data do aviso inválida — use o formato dd/mm/aaaa.";
  }
  if (dados.alarmeEm < dados.hoje) {
    return "A data do aviso não pode estar no passado.";
  }
  return null;
}
