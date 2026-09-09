<script setup lang="ts">
// Editor de acessos: define quais módulos (itens de menu) cada usuário
// pode ver. O admin tem acesso total e não aparece aqui como editável.
// As seções vêm do config/menu.ts — ao criar um módulo novo, ele já
// aparece automaticamente como opção de acesso.
import { computed, ref, watch } from "vue";
import { topMenu } from "../../config/menu";
import { mensagemDeErro } from "../../utils/erros";
import { useUsuarios } from "../../composables/useUsuarios";
import SelectBusca from "../ui/SelectBusca.vue";

const { usuarios, listarPermissoes, salvarPermissoes } = useUsuarios();

const selecionadoId = ref<number | null>(null);
const secoesSelecionadas = ref<string[]>([]);
const carregando = ref(false);
const salvando = ref(false);
const erro = ref<string | null>(null);
const sucesso = ref<string | null>(null);

/** Nome de exibição do usuário selecionado. */
const rotuloSelecionado = computed(
  () => usuarios.value.find((u) => u.id === selecionadoId.value),
);

const opcoesUsuarios = computed(() =>
  usuarios.value.map((usuario) => ({
    id: usuario.id,
    label: `${usuario.display_name} (${usuario.username})`,
  })),
);

/** Lista de ids de TODAS as seções do menu (para marcar/limpar tudo). */
const todasAsSecoes = computed(() => topMenu.flatMap((grupo) => grupo.items.map((i) => i.id)));

// Ao trocar o usuário selecionado, carrega os acessos atuais dele.
watch(selecionadoId, async (id) => {
  erro.value = null;
  sucesso.value = null;
  secoesSelecionadas.value = [];
  if (id === null) return;

  carregando.value = true;
  try {
    secoesSelecionadas.value = await listarPermissoes(id);
  } catch (err) {
    erro.value = mensagemDeErro(err);
  } finally {
    carregando.value = false;
  }
});

function alternarSecao(secaoId: string) {
  const indice = secoesSelecionadas.value.indexOf(secaoId);
  if (indice === -1) secoesSelecionadas.value.push(secaoId);
  else secoesSelecionadas.value.splice(indice, 1);
}

function marcarTodas() {
  secoesSelecionadas.value = [...todasAsSecoes.value];
}

function limparTodas() {
  secoesSelecionadas.value = [];
}

async function salvar() {
  if (selecionadoId.value === null) return;

  erro.value = null;
  sucesso.value = null;
  salvando.value = true;
  try {
    await salvarPermissoes(selecionadoId.value, secoesSelecionadas.value);
    sucesso.value = `Acessos de "${rotuloSelecionado.value?.display_name}" salvos com sucesso!`;
  } catch (err) {
    erro.value = mensagemDeErro(err);
  } finally {
    salvando.value = false;
  }
}
</script>

<template>
  <section
    class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
  >
    <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
      Acessos por usuário
    </h2>
    <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
      Escolha o usuário e marque os módulos que ele pode ver. O
      <code class="font-semibold">fernando</code> (superusuário) sempre tem
      acesso total.
    </p>

    <div class="mt-4 flex flex-col gap-1.5">
      <label for="acesso-usuario" class="text-sm font-medium">Usuário</label>
      <SelectBusca
        v-model="selecionadoId"
        :options="opcoesUsuarios"
        input-id="acesso-usuario"
        placeholder="Busque o usuário..."
      />
    </div>

    <!-- Aviso quando seleciona o superusuário -->
    <p
      v-if="rotuloSelecionado && rotuloSelecionado.username === 'fernando'"
      class="mt-4 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-sm text-indigo-700 dark:border-indigo-900 dark:bg-indigo-950 dark:text-indigo-300"
    >
      O usuário <strong>fernando</strong> é o superusuário: tem acesso a todos
      os módulos e não precisa de permissões.
    </p>

    <!-- Grade de permissões -->
    <div v-else-if="rotuloSelecionado" class="mt-4">
      <div class="flex items-center justify-between gap-2">
        <p class="text-sm text-neutral-600 dark:text-neutral-300">
          {{
            carregando
              ? "Carregando acessos..."
              : `${secoesSelecionadas.length} de ${todasAsSecoes.length} módulos liberados`
          }}
        </p>
        <div class="flex gap-1.5">
          <button
            type="button"
            @click="marcarTodas"
            class="rounded-lg px-2.5 py-1 text-xs font-medium text-indigo-600 transition hover:bg-indigo-50 dark:text-indigo-300 dark:hover:bg-indigo-950"
          >
            Marcar todos
          </button>
          <button
            type="button"
            @click="limparTodas"
            class="rounded-lg px-2.5 py-1 text-xs font-medium text-neutral-500 transition hover:bg-neutral-100 dark:text-neutral-400 dark:hover:bg-neutral-700"
          >
            Limpar
          </button>
        </div>
      </div>

      <div v-if="!carregando" class="mt-3 space-y-4">
        <fieldset v-for="grupo in topMenu" :key="grupo.label">
          <legend class="text-xs font-semibold uppercase tracking-wide text-neutral-400">
            {{ grupo.label }}
          </legend>
          <div class="mt-1.5 grid grid-cols-1 gap-1 sm:grid-cols-2">
            <label
              v-for="item in grupo.items"
              :key="item.id"
              class="flex cursor-pointer items-center gap-2 rounded-lg border border-neutral-100 px-2.5 py-1.5 text-sm transition hover:bg-neutral-50 dark:border-neutral-700/60 dark:hover:bg-neutral-700/40"
              :class="secoesSelecionadas.includes(item.id) ? 'border-indigo-200 bg-indigo-50/60 dark:border-indigo-900 dark:bg-indigo-950/30' : ''"
            >
              <input
                type="checkbox"
                :checked="secoesSelecionadas.includes(item.id)"
                @change="alternarSecao(item.id)"
                class="h-4 w-4 accent-indigo-600"
              />
              {{ item.label }}
            </label>
          </div>
        </fieldset>
      </div>

      <p
        v-if="erro"
        role="alert"
        class="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
      >
        {{ erro }}
      </p>
      <p
        v-if="sucesso"
        role="status"
        class="mt-3 rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-sm text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300"
      >
        {{ sucesso }}
      </p>

      <button
        type="button"
        :disabled="salvando"
        @click="salvar"
        class="mt-4 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {{ salvando ? "Salvando..." : "Salvar acessos" }}
      </button>
    </div>

    <p
      v-else
      class="mt-4 rounded-lg border border-dashed border-neutral-300 p-6 text-center text-sm text-neutral-400 dark:border-neutral-600"
    >
      Selecione um usuário acima para definir os módulos que ele pode acessar.
    </p>
  </section>
</template>
