import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/**
 * Usuário autenticado + sessão persistente ("Manter-me conectado").
 *
 * Fluxo:
 *  - login com `lembrar` → o backend cria um token de sessão (tabela
 *    `sessions`) e nós guardamos o token no localStorage do app;
 *  - ao reabrir o app, `restoreSession()` envia o token salvo ao backend
 *    (`autenticar_por_token`) e o usuário entra direto;
 *  - "Sair" envia o token para o backend encerrar a sessão e limpa o token.
 */
export interface User {
  id: number;
  username: string;
  display_name: string;
}

/** Resposta do comando login (usuário + token opcional). */
interface RespostaLogin {
  user: User;
  token: string | null;
}

// Chave usada no localStorage para guardar o token da sessão.
const CHAVE_TOKEN = "scd.token";

// Estado compartilhado em nível de módulo (composable "singleton"):
// como estes `ref`s vivem fora da função, TODOS os componentes que
// chamarem useAuth() enxergam o MESMO estado de autenticação.
const currentUser = ref<User | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
/** true enquanto o app confere se há sessão salva (evita "piscar" o login). */
const verificandoSessao = ref(true);

export function useAuth() {
  const isAuthenticated = computed(() => currentUser.value !== null);

  /** Chama o comando Rust "login". Com `lembrar`, guarda a sessão. */
  async function login(username: string, password: string, lembrar: boolean) {
    loading.value = true;
    error.value = null;
    try {
      const resposta = await invoke<RespostaLogin>("login", {
        username: username.trim(),
        password,
        lembrar,
      });
      currentUser.value = resposta.user;

      if (lembrar && resposta.token) {
        localStorage.setItem(CHAVE_TOKEN, resposta.token);
      } else {
        localStorage.removeItem(CHAVE_TOKEN);
      }
    } catch (err) {
      // Comandos Rust que retornam Err chegam aqui como string.
      error.value = typeof err === "string" ? err : "Não foi possível entrar.";
    } finally {
      loading.value = false;
    }
  }

  /** Chama o comando Rust "logout" e limpa a sessão local. */
  async function logout() {
    const token = localStorage.getItem(CHAVE_TOKEN);
    try {
      await invoke("logout", { token });
    } finally {
      localStorage.removeItem(CHAVE_TOKEN);
      currentUser.value = null;
      error.value = null;
    }
  }

  /**
   * Restaura a sessão ao abrir o aplicativo (se o usuário marcou
   * "Manter-me conectado", ele volta direto para o sistema).
   */
  async function restoreSession() {
    verificandoSessao.value = true;
    try {
      const token = localStorage.getItem(CHAVE_TOKEN);
      if (!token) {
        currentUser.value = null;
        return;
      }
      currentUser.value = await invoke<User | null>("autenticar_por_token", { token });
      // Token não é mais válido (sessão encerrada/removida) → limpa.
      if (!currentUser.value) localStorage.removeItem(CHAVE_TOKEN);
    } catch {
      localStorage.removeItem(CHAVE_TOKEN);
      currentUser.value = null;
    } finally {
      verificandoSessao.value = false;
    }
  }

  return {
    currentUser,
    isAuthenticated,
    loading,
    error,
    verificandoSessao,
    login,
    logout,
    restoreSession,
  };
}
