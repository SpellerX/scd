import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Sincronização com a nuvem (Supabase): estado compartilhado + chamadas aos
 * comandos Rust (src-tauri/src/commands/sync.rs). Mesmo padrão dos demais
 * composables: refs em nível de módulo.
 *
 * A sincronização é por DISPOSITIVO (uma "conta dona" por empresa), independente
 * do usuário local logado — o menu decide quem pode configurá-la.
 */

/** Estado da sincronização (espelho do struct Rust `EstadoSync`). */
export interface EstadoSync {
  conectado: boolean;
  email: string | null;
  ultima_sync: string | null;
}

/** Resumo de um ciclo (espelho do struct Rust `ResumoSync`). */
export interface ResumoSync {
  enviados: number;
  recebidos: number;
  avisos: string[];
}

/** Dados para conectar a conta da nuvem (chaves em camelCase → Rust). */
export interface ConfiguracaoConexao {
  projetoUrl: string;
  anonKey: string;
  email: string;
  senha: string;
}

const estado = ref<EstadoSync | null>(null);
const carregandoEstado = ref(false);
const sincronizando = ref(false);
const erro = ref<string | null>(null);
const ultimoResumo = ref<ResumoSync | null>(null);

export function useSync() {
  /** Lê a situação da conta da nuvem neste dispositivo. */
  async function carregarEstado() {
    carregandoEstado.value = true;
    try {
      estado.value = await invoke<EstadoSync>("estado_sync");
    } catch (err) {
      erro.value = typeof err === "string" ? err : "Não foi possível ler o estado da sincronização.";
    } finally {
      carregandoEstado.value = false;
    }
  }

  /** Conecta (valida e salva) a conta da nuvem. */
  async function conectar(config: ConfiguracaoConexao) {
    erro.value = null;
    await invoke<void>("conectar_sync", { config });
    await carregarEstado();
  }

  /** Remove a conta e os marcadores deste dispositivo. */
  async function desconectar() {
    erro.value = null;
    await invoke<void>("desconectar_sync");
    estado.value = null;
    ultimoResumo.value = null;
  }

  /** Executa um ciclo completo de sincronização (push + pull). */
  async function sincronizarAgora(): Promise<ResumoSync> {
    erro.value = null;
    sincronizando.value = true;
    try {
      ultimoResumo.value = await invoke<ResumoSync>("sincronizar_agora");
      await carregarEstado();
      return ultimoResumo.value;
    } catch (err) {
      erro.value = typeof err === "string" ? err : "Não foi possível sincronizar com a nuvem.";
      throw err;
    } finally {
      sincronizando.value = false;
    }
  }

  /**
   * Auto-sincronização: roda em segundo plano (intervalo do Dashboard).
   * Se não há conta conectada, não faz nada; falhas são silenciosas.
   */
  async function verificarEExecutar() {
    try {
      // Sempre tenta sincronizar: se ainda não há conta salva, o backend
      // conecta sozinho usando a configuração do .env ou a embutida no build.
      const resumo = await invoke<ResumoSync>("sincronizar_agora");
      ultimoResumo.value = resumo;
      estado.value = await invoke<EstadoSync>("estado_sync");
    } catch {
      // Sem configuração, sem internet ou sessão expirada → tenta de novo no
      // próximo ciclo; só atualiza o estado para a tela refletir a realidade.
      try {
        estado.value = await invoke<EstadoSync>("estado_sync");
      } catch {
        // estado_sync não falha em condições normais — ignora.
      }
    }
  }

  return {
    estado,
    carregandoEstado,
    sincronizando,
    erro,
    ultimoResumo,
    carregarEstado,
    conectar,
    desconectar,
    sincronizarAgora,
    verificarEExecutar,
  };
}
