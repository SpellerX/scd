<script setup lang="ts">
/**
 * SelectBusca: combobox de seleção com campo de busca.
 * Pensado para listas longas (ex.: escolher uma empresa entre muitas):
 * o usuário digita parte do nome e as opções são filtradas na hora.
 *
 * Uso:
 *   <SelectBusca v-model="empresaId" :options="opcoes" placeholder="..." />
 *
 * Modelo: o id selecionado (number | null). Opções: { id, label }.
 * Genérico e reutilizável — mesmas regras de UX em qualquer tela.
 */
import { computed, ref } from "vue";
import { normalizarTexto } from "../../utils/texto";

interface Opcao {
  id: number;
  label: string;
}

const props = withDefaults(
  defineProps<{
    options: Opcao[];
    placeholder?: string;
    inputId?: string;
  }>(),
  { placeholder: "Selecione...", inputId: undefined },
);

const valor = defineModel<number | null>({ default: null });

const aberto = ref(false);
/** Texto digitado para filtrar (vazio = mostra todas). */
const busca = ref("");
/** Índice da opção destacada no teclado (-1 = nenhuma). */
const indiceDestacado = ref(-1);

/** Opções filtradas pelo texto digitado (ignora acentos e maiúsculas). */
const filtradas = computed(() => {
  const termo = normalizarTexto(busca.value);
  if (!termo) return props.options;
  return props.options.filter((opcao) => normalizarTexto(opcao.label).includes(termo));
});

const rotuloSelecionado = computed(
  () => props.options.find((opcao) => opcao.id === valor.value)?.label ?? "",
);

/** Texto exibido no input: o que o usuário digita OU o rótulo selecionado. */
const textoExibido = computed(() => {
  if (aberto.value || busca.value !== "") return busca.value;
  return rotuloSelecionado.value;
});

function abrir() {
  aberto.value = true;
  indiceDestacado.value = filtradas.value.length > 0 ? 0 : -1;
}

function fechar() {
  aberto.value = false;
  busca.value = "";
  indiceDestacado.value = -1;
}

function selecionar(opcao: Opcao) {
  valor.value = opcao.id;
  fechar();
}

function destacar(deslocamento: number) {
  const total = filtradas.value.length;
  if (total === 0) return;
  const atual = indiceDestacado.value === -1 ? 0 : indiceDestacado.value;
  indiceDestacado.value = (atual + deslocamento + total) % total;
}

function aoDigitar(evento: Event) {
  busca.value = (evento.target as HTMLInputElement).value;
  indiceDestacado.value = filtradas.value.length > 0 ? 0 : -1;
}

function aoTeclar(evento: KeyboardEvent) {
  // Navegação com setas dentro do dropdown
  if (evento.key === "ArrowDown") {
    evento.preventDefault();
    if (!aberto.value) abrir();
    else destacar(1);
    return;
  }
  if (evento.key === "ArrowUp") {
    evento.preventDefault();
    if (aberto.value) destacar(-1);
    return;
  }
  if (evento.key === "Enter") {
    // Com o dropdown aberto, Enter escolhe a opção destacada (não envia formulário)
    if (aberto.value) {
      evento.preventDefault();
      const escolha = filtradas.value[indiceDestacado.value];
      if (escolha) selecionar(escolha);
    }
    return;
  }
  if (evento.key === "Escape") {
    fechar();
    return;
  }
  if (evento.key === "Tab") {
    fechar();
  }
}

function limpar() {
  valor.value = null;
  busca.value = "";
}
</script>

<template>
  <div class="relative">
    <input
      :id="inputId"
      type="text"
      role="combobox"
      :aria-expanded="aberto"
      :value="textoExibido"
      :placeholder="placeholder"
      autocomplete="off"
      @focus="abrir"
      @input="aoDigitar"
      @keydown="aoTeclar"
      class="w-full rounded-lg border border-neutral-300 bg-white px-3.5 py-2 pr-9 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
    />

    <!-- Limpar a seleção atual -->
    <button
      v-if="valor !== null && !aberto"
      type="button"
      :aria-label="'Limpar seleção de ' + (rotuloSelecionado || 'opção')"
      @click="limpar"
      class="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 text-neutral-400 transition hover:text-neutral-700 dark:hover:text-neutral-200"
    >
      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="h-4 w-4">
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
      </svg>
    </button>

    <!-- Camada invisível que fecha o dropdown ao clicar fora -->
    <div v-if="aberto" class="fixed inset-0 z-10" @click="fechar" />

    <!-- Dropdown com as opções filtradas -->
    <ul
      v-if="aberto"
      role="listbox"
      class="absolute left-0 right-0 top-full z-20 mt-1.5 max-h-64 overflow-y-auto rounded-xl border border-neutral-200 bg-white p-1.5 shadow-lg dark:border-neutral-700 dark:bg-neutral-800"
    >
      <li
        v-if="filtradas.length === 0"
        class="px-3 py-2 text-sm text-neutral-400 dark:text-neutral-500"
      >
        Nenhum resultado para "{{ busca }}"
      </li>
      <li v-for="(opcao, indice) in filtradas" :key="opcao.id">
        <button
          type="button"
          role="option"
          :aria-selected="opcao.id === valor"
          @mousedown.prevent
          @click="selecionar(opcao)"
          class="w-full rounded-lg px-3 py-2 text-left text-sm transition"
          :class="
            indice === indiceDestacado
              ? 'bg-indigo-50 font-medium text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300'
              : 'text-neutral-700 hover:bg-neutral-100 dark:text-neutral-200 dark:hover:bg-neutral-700'
          "
        >
          {{ opcao.label }}
        </button>
      </li>
    </ul>
  </div>
</template>
