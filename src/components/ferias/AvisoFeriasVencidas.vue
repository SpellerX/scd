<script setup lang="ts">
// Popup de ação para férias VENCIDAS:
//   - "Regularizar agora" → abre o sistema na página Férias vencidas;
//   - "Lembrar mais tarde" → esconde o popup até o dia seguinte.
// Aparece ao abrir o app (inclusive quando o usuário clica na notificação
// nativa do Windows e o app volta ao foco) e some quando não há pendências.
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useFerias } from "../../composables/useFerias";
import { useNavigation } from "../../composables/useNavigation";
import { usePermissoes } from "../../composables/usePermissoes";
import { topMenu } from "../../config/menu";
import Icone from "../ui/Icone.vue";

const CHAVE_SONECA = "scd.avisoFerias.soneca";

const { vencidas, carregar } = useFerias();
const { openSection } = useNavigation();
const { pode } = usePermissoes();

const visivel = ref(false);
let intervalo: number | undefined;

/** Seção "Férias vencidas" (alvo do botão Regularizar agora). */
const secaoVencidas = computed(() =>
  topMenu.flatMap((g) => g.items).find((item) => item.id === "cadastro-ferias-vencidas"),
);

/** Hoje em yyyy-mm-dd (local). */
function hojeIso(): string {
  const d = new Date();
  const mes = String(d.getMonth() + 1).padStart(2, "0");
  const dia = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mes}-${dia}`;
}

function sonecaAtiva(): boolean {
  return localStorage.getItem(CHAVE_SONECA) === hojeIso();
}

/** Reavalia se o popup deve aparecer. */
function reavaliar() {
  const existeVencida = vencidas.value.length > 0;
  const podeVerSecao = secaoVencidas.value ? pode(secaoVencidas.value.id) : false;
  visivel.value = existeVencida && podeVerSecao && !sonecaAtiva();
}

/** "Lembrar mais tarde": esconde o popup E pausa os cards nativos até amanhã. */
function lembrarMaisTarde() {
  localStorage.setItem(CHAVE_SONECA, hojeIso());
  visivel.value = false;
  // Pede ao backend para não reenviar notificações nativas hoje.
  void invoke("sonecar_notificacoes_os").catch(() => undefined);
}

/** "Regularizar agora": abre o sistema na página de férias vencidas. */
function regularizarAgora() {
  if (secaoVencidas.value) openSection(secaoVencidas.value);
  visivel.value = false;
}

onMounted(async () => {
  await carregar();
  reavaliar();
  // Mantém o aviso atualizado (se regularizar em outra tela, o popup fecha).
  intervalo = window.setInterval(() => {
    void carregar().then(reavaliar);
  }, 60_000);
});

onUnmounted(() => {
  if (intervalo !== undefined) window.clearInterval(intervalo);
});
</script>

<template>
  <!-- Popup central de ação -->
  <Transition name="modal">
    <div
      v-if="visivel"
      role="dialog"
      aria-modal="true"
      aria-label="Férias vencidas precisam de atenção"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
    >
      <div
        class="w-full max-w-md rounded-2xl bg-white p-6 shadow-2xl dark:bg-neutral-800"
      >
        <div class="flex items-start gap-4">
          <span
            class="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-red-100 text-red-600 dark:bg-red-950 dark:text-red-400"
          >
            <Icone nome="ferias-vencidas" tamanho="md" />
          </span>
          <div class="min-w-0">
            <h2 class="text-base font-bold">Férias vencidas precisam de atenção</h2>
            <p class="mt-1 text-sm text-neutral-600 dark:text-neutral-300">
              {{ vencidas.length }} pendência(s) — ex.:
              <span class="font-semibold">{{ vencidas[0]?.funcionario_nome }}</span>
              ({{ vencidas[0]?.empresa_nome }}), vencida(s) há
              {{ vencidas[0]?.dias_vencidas }} dia(s).
            </p>
          </div>
        </div>

        <div class="mt-6 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button
            type="button"
            @click="lembrarMaisTarde"
            class="rounded-lg border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 transition hover:bg-neutral-100 active:scale-95 dark:border-neutral-600 dark:text-neutral-200 dark:hover:bg-neutral-700"
          >
            Lembrar mais tarde
          </button>
          <button
            type="button"
            @click="regularizarAgora"
            class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow transition hover:bg-indigo-500 active:scale-95"
          >
            Regularizar agora
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
/* Fundo escurece/some suavemente */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.25s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

/* Cartão sobe levemente com efeito "mola" ao abrir */
.modal-enter-active > div,
.modal-leave-active > div {
  transition: transform 0.28s cubic-bezier(0.34, 1.3, 0.4, 1);
}
.modal-enter-from > div,
.modal-leave-to > div {
  transform: translateY(18px) scale(0.95);
}
</style>
