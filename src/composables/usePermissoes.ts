import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Permissões do usuário logado: quais seções (itens de menu) ele pode ver.
 *
 * - `fernando` (superusuário) tem acesso a TUDO — ignora a tabela de permissões;
 * - demais usuários: a lista de seções vem do backend (Sistema → Usuários);
 * - o menu superior é filtrado por `pode(id)`.
 */
const usuarioAdmin = ref(false);
const secoesPermitidas = ref<string[]>([]);
/** false enquanto as permissões ainda não foram carregadas. */
const pronto = ref(false);

export function usePermissoes() {
  /** Carrega as permissões do usuário que acabou de entrar (null = deslogado). */
  async function carregar(usuario: { id: number; username: string } | null) {
    pronto.value = false;
    if (!usuario) {
      usuarioAdmin.value = false;
      secoesPermitidas.value = [];
      pronto.value = true;
      return;
    }

    usuarioAdmin.value = usuario.username === "fernando";
    if (usuarioAdmin.value) {
      secoesPermitidas.value = [];
      pronto.value = true;
      return;
    }

    try {
      secoesPermitidas.value = await invoke<string[]>("listar_permissoes_do_usuario", {
        userId: usuario.id,
      });
    } catch {
      secoesPermitidas.value = [];
    } finally {
      pronto.value = true;
    }
  }

  /** O usuário logado pode ver esta seção? */
  function pode(secaoId: string): boolean {
    if (usuarioAdmin.value) return true;
    if (!pronto.value) return false; // ainda carregando → nada liberado (evita vazamento)
    return secoesPermitidas.value.includes(secaoId);
  }

  return { usuarioAdmin, secoesPermitidas, pronto, carregar, pode };
}
