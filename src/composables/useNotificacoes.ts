import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Notificações internas do app (sino no cabeçalho).
 * São derivadas das férias: períodos VENCIDOS e alertas de períodos
 * prestes a vencer (dentro dos dias configurados).
 *
 * As notificações voltam a aparecer enquanto a pendência existir; o botão
 * "marcar como lidas" apenas esconde as atuais (ids guardados no navegador).
 */
export interface Notificacao {
  id: string;
  tipo: "vencida" | "alerta";
  titulo: string;
  mensagem: string;
}

const CHAVE_LIDAS = "scd.notificacoes.lidas";

/** Ids já "lidas" (ocultas) nesta sessão. */
const lidas = ref<Set<string>>(new Set(JSON.parse(localStorage.getItem(CHAVE_LIDAS) ?? "[]")));
const notificacoes = ref<Notificacao[]>([]);
const carregando = ref(false);

function persistirLidas() {
  localStorage.setItem(CHAVE_LIDAS, JSON.stringify([...lidas.value]));
}

export function useNotificacoes() {
  /** Busca as notificações atuais no backend. */
  async function carregar() {
    carregando.value = true;
    try {
      notificacoes.value = await invoke<Notificacao[]>("listar_notificacoes");
    } catch {
      notificacoes.value = [];
    } finally {
      carregando.value = false;
    }
  }

  /** Notificações ainda visíveis (não marcadas como lidas). */
  const visiveis = computed(() => notificacoes.value.filter((n) => !lidas.value.has(n.id)));

  /** Oculta as notificações atuais (mantém a pendência no banco). */
  function marcarTodasComoLidas() {
    for (const n of notificacoes.value) lidas.value.add(n.id);
    persistirLidas();
  }

  return { notificacoes, visiveis, carregando, carregar, marcarTodasComoLidas };
}
