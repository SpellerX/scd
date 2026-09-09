import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Controle de férias: listas (vencidas / a vencer), regularização e a
 * configuração do alerta. Espelha os comandos em commands/ferias.rs.
 */

/** Período vencido (espelho de `FeriasVencida`). */
export interface FeriasVencida {
  id: number;
  funcionario_id: number;
  funcionario_nome: string;
  empresa_nome: string;
  inicio: string;
  vencimento: string;
  dias_vencidas: number;
  observacao: string | null;
}

/** Período a vencer (espelho de `FeriasAVencer`). */
export interface FeriasAVencer {
  id: number;
  funcionario_id: number;
  funcionario_nome: string;
  empresa_nome: string;
  inicio: string;
  vencimento: string;
  dias_restantes: number;
  dentro_alerta: boolean;
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
  function regularizar(id: number, observacao: string | null): Promise<void> {
    return invoke<void>("regularizar_ferias", { id, observacao });
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
    obterAlertaDias,
    definirAlertaDias,
  };
}
