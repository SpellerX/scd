<script setup lang="ts">
// Área autenticada do SCD (dashboard): cabeçalho com o menu superior
// (TopMenu) + área de conteúdo que troca conforme a seção ativa.
//
// O menu é filtrado pelas PERMISSÕES do usuário logado (usePermissoes):
// - admin vê tudo; os demais veem apenas as seções liberadas em
//   Sistema → Usuários → Acessos.
// As seções com tela própria ficam registradas em `telasDasSecoes`;
// as demais mostram o placeholder "em construção".
import { computed, onMounted, onUnmounted, type Component } from "vue";
import { listen } from "@tauri-apps/api/event";
import { topMenu } from "../config/menu";
import { useAuth } from "../composables/useAuth";
import { useNavigation } from "../composables/useNavigation";
import { usePermissoes } from "../composables/usePermissoes";
import { useSync } from "../composables/useSync";
import TopMenu from "../components/navigation/TopMenu.vue";
import NotificacoesSino from "../components/navigation/NotificacoesSino.vue";
import AvisoFeriasVencidas from "../components/ferias/AvisoFeriasVencidas.vue";
import IndicadoresInicio from "../components/dashboard/IndicadoresInicio.vue";
import EmpresaView from "./sections/EmpresaView.vue";
import FuncionariosView from "./sections/FuncionariosView.vue";
import UsuariosView from "./sections/UsuariosView.vue";
import FeriasVencidasView from "./sections/FeriasVencidasView.vue";
import FeriasAVencerView from "./sections/FeriasAVencerView.vue";
import SincronizacaoView from "./sections/SincronizacaoView.vue";

const { currentUser, logout } = useAuth();
const { activeSection, goHome, openSection } = useNavigation();
const { usuarioAdmin, pronto: permissoesProntas, secoesPermitidas, pode } = usePermissoes();
const { verificarEExecutar: sincronizarSeConectado } = useSync();

/** Usuário logado não é admin e não tem nenhum módulo liberado ainda. */
const semModulosLiberados = computed(
  () =>
    !usuarioAdmin.value &&
    permissoesProntas.value &&
    secoesPermitidas.value.length === 0,
);

/**
 * Menu visível para o usuário logado: itens permitidos em cada grupo
 * (grupos sem itens permitidos ficam ocultos).
 */
const menuFiltrado = computed(() =>
  topMenu
    .map((grupo) => ({
      ...grupo,
      items: grupo.items.filter((item) => pode(item.id)),
    }))
    .filter((grupo) => grupo.items.length > 0),
);

/**
 * Registro de telas por seção: id definido em src/config/menu.ts → view.
 * Quando uma nova seção ganhar tela própria, adicione a entrada aqui.
 */
const telasDasSecoes: Record<string, Component> = {
  "cadastro-empresa": EmpresaView,
  "cadastro-funcionarios": FuncionariosView,
  "cadastro-ferias-vencidas": FeriasVencidasView,
  "ferias-a-vencer": FeriasAVencerView,
  "sistema-usuarios": UsuariosView,
  "sistema-sincronizacao": SincronizacaoView,
};

/** Seção ativa que o usuário realmente pode ver (senão volta ao dashboard). */
const secaoVisivel = computed(() => {
  const secao = activeSection.value;
  if (!secao) return null;
  return pode(secao.id) ? secao : null;
});

const secaoComponent = computed<Component | null>(() => {
  const secao = secaoVisivel.value;
  return secao ? (telasDasSecoes[secao.id] ?? null) : null;
});

// Evento vindo da JANELA DE NOTIFICAÇÃO (ou de outros pontos): navega para a
// seção correta, ex.: clicou em "Regularizar agora" → abre Férias vencidas.
let desinscreverEvento: (() => void) | undefined;
let timerSincronizacao: number | undefined;

// Auto-sincronização com a nuvem: enquanto o app estiver aberto, tenta um
// ciclo a cada 60 s (sem conta conectada, o comando não faz nada).
const INTERVALO_SINCRONIZACAO_MS = 60_000;

onMounted(() => {
  void listen<{ secao: string }>("ir-para-secao", (evento) => {
    const item = topMenu.flatMap((grupo) => grupo.items).find((i) => i.id === evento.payload.secao);
    if (item) openSection(item);
  }).then((desinscrever) => {
    desinscreverEvento = desinscrever;
  });

  // Primeira checagem logo ao entrar; depois em intervalos regulares.
  void sincronizarSeConectado();
  timerSincronizacao = window.setInterval(() => {
    void sincronizarSeConectado();
  }, INTERVALO_SINCRONIZACAO_MS);
});
onUnmounted(() => {
  desinscreverEvento?.();
  if (timerSincronizacao !== undefined) window.clearInterval(timerSincronizacao);
});
</script>

<template>
  <div class="flex flex-1 flex-col">
    <!-- ════════ Cabeçalho / menu superior ════════ -->
    <header
      class="sticky top-0 z-30 border-b border-neutral-200 bg-white dark:border-neutral-800 dark:bg-neutral-900"
    >
      <div class="mx-auto flex h-14 w-full max-w-6xl items-center gap-4 px-4">
        <!-- Marca: clicar volta para a tela inicial -->
        <button
          type="button"
          class="flex shrink-0 items-center gap-2 rounded-lg focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          @click="goHome"
          title="Ir para o início"
        >
          <span
            class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-600 to-violet-600 text-xs font-bold tracking-wider text-white shadow"
          >
            SCD
          </span>
          <span class="hidden text-sm font-semibold sm:block">SCD</span>
        </button>

        <!-- Menu superior (filtrado por permissões) -->
        <TopMenu :groups="menuFiltrado" />

        <!-- Espaço flexível -->
        <div class="flex-1" />

        <!-- Notificações internas (férias vencidas / alertas) -->
        <NotificacoesSino />

        <!-- Usuário da sessão + sair -->
        <span class="hidden text-sm text-neutral-500 md:block dark:text-neutral-400">
          {{ currentUser?.display_name }}
        </span>
        <button
          type="button"
          @click="logout"
          class="rounded-lg border border-neutral-300 px-3 py-1.5 text-sm font-medium text-neutral-700 transition hover:bg-neutral-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:border-neutral-600 dark:text-neutral-200 dark:hover:bg-neutral-800"
        >
          Sair
        </button>
      </div>
    </header>

    <!-- ════════ Conteúdo ════════ -->
    <main class="mx-auto w-full max-w-6xl flex-1 p-6">
      <!-- Tela inicial: dashboard de indicadores -->
      <template v-if="!secaoVisivel">
        <!-- Aviso para usuário sem módulos liberados -->
        <p
          v-if="semModulosLiberados"
          role="status"
          class="mb-6 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-center text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200"
        >
          Você ainda não tem módulos liberados. Peça ao administrador para
          definir seus acessos em Sistema → Usuários.
        </p>
        <IndicadoresInicio />
      </template>

      <!-- Tela real da seção escolhida (ver telasDasSecoes) -->
      <component :is="secaoComponent" v-else-if="secaoComponent" />

      <!-- Placeholder das seções que ainda não têm tela própria -->
      <section
        v-else
        class="mx-auto mt-8 max-w-xl rounded-2xl border border-dashed border-neutral-300 bg-white p-10 text-center shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
      >
        <h2 class="text-xl font-bold">{{ secaoVisivel?.label }}</h2>
        <p class="mt-2 text-neutral-500 dark:text-neutral-400">
          {{ secaoVisivel?.description }}
        </p>
        <span
          class="mt-5 inline-block rounded-full bg-amber-100 px-3 py-1 text-xs font-medium text-amber-800 dark:bg-amber-900 dark:text-amber-200"
        >
          Módulo em construção
        </span>
      </section>
    </main>

    <!-- Popup de ação: férias vencidas (Regularizar agora / Lembrar mais tarde) -->
    <AvisoFeriasVencidas />
  </div>
</template>
