<script setup lang="ts">
// Componente raiz (shell) do aplicativo.
// Aqui mora o "esqueleto" global (tema claro/escuro e layout base) e a
// escolha da tela conforme o estado de autenticação:
//   - verificando sessão salva → splash (evita "piscar" a tela de login);
//   - deslogado  → LoginView (tela inicial);
//   - autenticado → DashboardView (menu superior + conteúdo).
import { onMounted, watch } from "vue";
import { useAuth } from "./composables/useAuth";
import { usePermissoes } from "./composables/usePermissoes";
import { useAtualizacao } from "./composables/useAtualizacao";
import LoginView from "./views/LoginView.vue";
import DashboardView from "./views/DashboardView.vue";

const { isAuthenticated, verificandoSessao, restoreSession, currentUser } = useAuth();
const { carregar: carregarPermissoes } = usePermissoes();
const { verificarEAtualizar } = useAtualizacao();

// Ao abrir o app, confere se há uma sessão salva ("Manter-me conectado").
onMounted(() => {
  void restoreSession();
  // Checa atualizações depois que a interface estabiliza (3 s) — falhas
  // são silenciosas e nunca atrapalham o uso.
  window.setTimeout(() => {
    void verificarEAtualizar();
  }, 3_000);
});

// Sempre que o usuário muda (login, restauração de sessão ou logout),
// recarrega as permissões — o menu se ajusta ao que ele pode ver.
watch(currentUser, (usuario) => {
  void carregarPermissoes(usuario ?? null);
});
</script>

<template>
  <div
    class="flex min-h-screen flex-col bg-neutral-100 text-neutral-900 transition-colors dark:bg-neutral-900 dark:text-neutral-100"
  >
    <!-- Splash enquanto confere a sessão salva -->
    <div
      v-if="verificandoSessao"
      class="flex flex-1 items-center justify-center"
      role="status"
      aria-label="Carregando o SCD"
    >
      <div class="flex flex-col items-center gap-4">
        <span
          class="flex h-16 w-16 animate-pulse items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-600 to-violet-600 text-xl font-bold tracking-wider text-white shadow-lg"
        >
          SCD
        </span>
        <span class="text-sm text-neutral-400 dark:text-neutral-500">
          Sistema de Controle de Demandas
        </span>
      </div>
    </div>

    <template v-else>
      <LoginView v-if="!isAuthenticated" />
      <DashboardView v-else />
    </template>
  </div>
</template>
