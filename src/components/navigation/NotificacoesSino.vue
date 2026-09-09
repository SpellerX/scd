<script setup lang="ts">
// Sino de notificações internas (férias vencidas e alertas a vencer).
// Fica no cabeçalho; mostra um contador e a lista ao clicar.
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useNotificacoes } from "../../composables/useNotificacoes";

const { visiveis, carregar, marcarTodasComoLidas } = useNotificacoes();

const aberto = ref(false);
let intervalo: number | undefined;

/** Pede ao backend para disparar as notificações NATIVAS do Windows. */
async function verificarNotificacoesOs() {
  try {
    await invoke("verificar_notificacoes_os");
  } catch {
    // Sem plugin/OS indisponível: o sino interno continua funcionando.
  }
}

onMounted(async () => {
  await verificarNotificacoesOs();
  await carregar();
  // Mantém notificações (sino + nativas) razoavelmente atuais (60 s).
  intervalo = window.setInterval(() => {
    void verificarNotificacoesOs();
    void carregar();
  }, 60_000);
});

onUnmounted(() => {
  if (intervalo !== undefined) window.clearInterval(intervalo);
});
</script>

<template>
  <div class="relative">
    <!-- Botão do sino -->
    <button
      type="button"
      :aria-expanded="aberto"
      aria-label="Notificações"
      @click="aberto = !aberto"
      class="relative rounded-lg p-2 text-neutral-500 transition hover:bg-neutral-100 hover:text-neutral-900 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:text-neutral-300 dark:hover:bg-neutral-800 dark:hover:text-white"
    >
      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.8" stroke="currentColor" class="h-5 w-5">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          d="M14.857 17.082a23.848 23.848 0 0 0 5.454-1.31A8.967 8.967 0 0 1 18 9.75V9A6 6 0 0 0 6 9v.75a8.967 8.967 0 0 1-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24.255 24.255 0 0 1-5.714 0m5.714 0a3 3 0 1 1-5.714 0"
        />
      </svg>

      <!-- Contador -->
      <span
        v-if="visiveis.length > 0"
        class="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-red-500 px-1 text-[10px] font-bold text-white"
      >
        {{ visiveis.length > 99 ? "99+" : visiveis.length }}
      </span>
    </button>

    <!-- Camada que fecha ao clicar fora -->
    <Transition name="fade">
      <div v-if="aberto" class="fixed inset-0 z-10" @click="aberto = false" />
    </Transition>

    <!-- Painel de notificações -->
    <Transition name="painel">
      <div
        v-if="aberto"
        class="absolute right-0 top-full z-20 mt-1.5 w-96 max-w-[90vw] origin-top-right overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-lg dark:border-neutral-700 dark:bg-neutral-800"
      >
        <div class="flex items-center justify-between border-b border-neutral-100 px-4 py-2.5 dark:border-neutral-700/60">
          <h3 class="text-sm font-semibold">Notificações</h3>
          <button
            type="button"
            @click="marcarTodasComoLidas"
            class="text-xs font-medium text-indigo-600 transition hover:text-indigo-500 dark:text-indigo-300"
            :disabled="visiveis.length === 0"
          >
            Marcar todas como lidas
          </button>
        </div>

        <ul class="max-h-80 overflow-y-auto divide-y divide-neutral-100 dark:divide-neutral-700/60">
          <li v-for="notificacao in visiveis" :key="notificacao.id">
            <div class="flex gap-3 px-4 py-3">
              <span
                class="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg"
                :class="
                  notificacao.tipo === 'vencida'
                    ? 'bg-red-100 text-red-600 dark:bg-red-950 dark:text-red-400'
                    : 'bg-amber-100 text-amber-600 dark:bg-amber-950 dark:text-amber-400'
                "
              >
                <svg v-if="notificacao.tipo === 'vencida'" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2.2" stroke="currentColor" class="h-4 w-4">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
                </svg>
                <svg v-else xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="h-4 w-4">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
                  <path stroke-linecap="round" stroke-linejoin="round" d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
                </svg>
              </span>
              <span class="min-w-0">
                <span class="block text-sm font-semibold">{{ notificacao.titulo }}</span>
                <span class="block text-xs text-neutral-500 dark:text-neutral-400">
                  {{ notificacao.mensagem }}
                </span>
              </span>
            </div>
          </li>
        </ul>

        <p
          v-if="visiveis.length === 0"
          class="px-4 py-6 text-center text-sm text-neutral-400 dark:text-neutral-500"
        >
          Nenhuma notificação no momento. 🎉
        </p>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
/* Fundo de clique-fora */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Painel: abre do topo direito com leve escala */
.painel-enter-active {
  transition:
    opacity 0.22s ease,
    transform 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.15);
}
.painel-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}
.painel-enter-from {
  opacity: 0;
  transform: translateY(-8px) scale(0.96);
}
.painel-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.97);
}

/* Itens entram em cascata quando o painel abre */
.painel-enter-active li {
  animation: item-entrar 0.3s ease both;
}
.painel-enter-active li:nth-child(2) { animation-delay: 0.03s; }
.painel-enter-active li:nth-child(3) { animation-delay: 0.06s; }
.painel-enter-active li:nth-child(4) { animation-delay: 0.09s; }
.painel-enter-active li:nth-child(n + 5) { animation-delay: 0.12s; }

@keyframes item-entrar {
  from {
    opacity: 0;
    transform: translateX(10px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
</style>
