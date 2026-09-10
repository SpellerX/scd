import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Controle de férias: listas (vencidas / a vencer), regularização, o alarme
 * manual ("férias agendadas") e a configuração do alerta automático.
 * Espelha os comandos em commands/ferias.rs.
 */

/** Período vencido (espelho de `FeriasVencida`). */
export interface FeriasVencida {
  /** Id UUID do período — chave única global. */
  id: string;
  funcionario_id: string;
  funcionario_nome: string;
  empresa_nome: string;
  inicio: string;
  vencimento: string;
  dias_vencidas: number;
  observacao: string | null;
}

/** Período a vencer (espelho de `FeriasAVencer`). */
export interface FeriasAVencer {
  /** Id UUID do período — chave única global. */
  id: string;
  funcionario_id: string;
  funcionario_nome: string;
  empresa_nome: string;
  inicio: string;
  vencimento: string;
  dias_restantes: number;
  /** Caiu na janela automática (N dias antes do vencimento)? */
  dentro_alerta: boolean;
  /** Está no prazo automático de exibição (12 meses antes do vencimento)? */
  na_janela_automatica: boolean;
  /** Tem alarme manual (férias agendadas pela empresa)? */
  agendado: boolean;
  /** Data escolhida para o aviso (yyyy-mm-dd). */
  alarme_em: string | null;
  /** Texto livre do agendamento. */
  alarme_observacao: string | null;
  /** Dias até a data do aviso (negativo = atrasado). */
  dias_para_alarme: number | null;
  /** A data do aviso já chegou (aviso pendente)? */
  alarme_disparado: boolean;
}

/** Período de um funcionário, usado no agendamento (espelho `PeriodoDoFuncionario`). */
export interface PeriodoDoFuncionario {
  id: string;
  inicio: string;
  vencimento: string;
  dias_restantes: number;
  alarme_em: string | null;
  alarme_observacao: string | null;
}

const vencidas = ref<FeriasVencida[]>([]);
const aVencer = ref<FeriasAVencer[]>([]);
const carregando = ref(false);
const erro = ref<string | null>(null);

export function useFerias() {
  /** Recarrega as duas listas. */
  async function carregar() {
    carregando.value = true;
    erro.value = null;
    try {
      const [v, a] = await Promise.all([
        invoke<FeriasVencida[]>("listar_ferias_vencidas"),
        invoke<FeriasAVencer[]>("listar_ferias_a_vencer"),
      ]);
      vencidas.value = v;
      aVencer.value = a;
    } catch (err) {
      erro.value = typeof err === "string" ? err : "Não foi possível carregar as férias.";
    } finally {
      carregando.value = false;
    }
  }

  /** Registra que o funcionário já gozou/quitou o período vencido. */
  function regularizar(id: string, observacao: string | null): Promise<void> {
    return invoke<void>("regularizar_ferias", { id, observacao });
  }

  /** Períodos de um funcionário que ainda podem ser agendados. */
  function listarPeriodosFuncionario(funcionarioId: string): Promise<PeriodoDoFuncionario[]> {
    return invoke<PeriodoDoFuncionario[]>("listar_periodos_funcionario", {
      funcionarioId,
    });
  }

  /**
   * Define (ou substitui) o alarme manual de um período: `alarmeEm` (yyyy-mm-dd)
   * é a data em que o aviso deve aparecer — o período entra na lista na hora,
   * sem esperar o alerta automático.
   */
  function definirAlarme(
    periodoId: string,
    alarmeEm: string,
    observacao: string | null,
  ): Promise<void> {
    return invoke<void>("definir_alarme_ferias", { periodoId, alarmeEm, observacao });
  }

  /** Remove o alarme de um período (o alerta automático volta a valer). */
  function removerAlarme(periodoId: string): Promise<void> {
    return invoke<void>("remover_alarme_ferias", { periodoId });
  }

  /** Dias de antecedência do alerta configurados. */
  function obterAlertaDias(): Promise<number> {
    return invoke<number>("obter_alerta_ferias");
  }

  /** Define os dias de antecedência do alerta. */
  function definirAlertaDias(dias: number): Promise<void> {
    return invoke<void>("definir_alerta_ferias", { dias });
  }

  return {
    vencidas,
    aVencer,
    carregando,
    erro,
    carregar,
    regularizar,
    listarPeriodosFuncionario,
    definirAlarme,
    removerAlarme,
    obterAlertaDias,
    definirAlertaDias,
  };
}
