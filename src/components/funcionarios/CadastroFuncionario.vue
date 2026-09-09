<script setup lang="ts">
// Formulário de cadastro de funcionário, já vinculando a uma empresa.
// Os nomes das empresas vêm do estado compartilhado de useEmpresas.
import { computed, ref } from "vue";
import { formatarCpf } from "../../utils/cpf";
import { apenasDigitos } from "../../utils/digitos";
import { mensagemDeErro } from "../../utils/erros";
import { useEmpresas } from "../../composables/useEmpresas";
import { useFuncionarios } from "../../composables/useFuncionarios";
import SelectBusca from "../ui/SelectBusca.vue";

const { empresas } = useEmpresas();
const { criar, listar: listarFuncionarios } = useFuncionarios();

const nome = ref("");
const digitosCpf = ref("");
const empresaId = ref<string | null>(null);
const dataAdmissao = ref("");

const salvando = ref(false);
const erro = ref<string | null>(null);
const sucesso = ref<string | null>(null);

// Entrada do CPF com máscara: guardamos só os dígitos e exibimos formatado.
const cpfExibido = computed({
  get: () => formatarCpf(digitosCpf.value),
  set: (valor) => {
    digitosCpf.value = apenasDigitos(valor);
  },
});

// Opções do SelectBusca (formato { id, label }).
const opcoesEmpresas = computed(() =>
  empresas.value.map((empresa) => ({
    id: empresa.id,
    label: empresa.razao_social,
  })),
);

const podeSalvar = computed(
  () =>
    nome.value.trim() !== "" &&
    dataAdmissao.value !== "" &&
    empresaId.value !== null &&
    (digitosCpf.value.length === 0 || digitosCpf.value.length === 11),
);

async function salvar() {
  erro.value = null;
  sucesso.value = null;

  if (!podeSalvar.value) {
    erro.value =
      "Preencha o nome, a data de admissão e escolha a empresa (CPF é opcional; se informado, use 11 dígitos).";
    return;
  }

  salvando.value = true;
  try {
    const funcionario = await criar({
      nome: nome.value.trim(),
      cpf: digitosCpf.value,
      // Chaves do invoke em camelCase (padrão Tauri → snake_case no Rust).
      empresaId: empresaId.value!,
      dataAdmissao: dataAdmissao.value || null,
    });
    sucesso.value = `"${funcionario.nome}" foi cadastrado(a) em ${funcionario.empresa_nome}.`;

    // Limpa o formulário e atualiza a listagem abaixo.
    nome.value = "";
    digitosCpf.value = "";
    empresaId.value = null;
    dataAdmissao.value = "";
    await listarFuncionarios();
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
      Cadastrar funcionário
    </h2>
    <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
      Vincula um colaborador a uma empresa (base para o controle de férias).
    </p>

    <!-- Aviso quando ainda não há empresa para vincular -->
    <p
      v-if="empresas.length === 0"
      class="mt-4 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200"
    >
      Cadastre ao menos uma empresa (menu Cadastro → Empresa) antes de criar
      funcionários.
    </p>

    <form v-else class="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2" @submit.prevent="salvar">
      <div class="flex flex-col gap-1.5 sm:col-span-2">
        <label for="func-nome" class="text-sm font-medium">Nome completo *</label>
        <input
          id="func-nome"
          v-model="nome"
          type="text"
          placeholder="Nome do funcionário"
          required
          class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <label for="func-cpf" class="text-sm font-medium">CPF (opcional)</label>
        <input
          id="func-cpf"
          v-model="cpfExibido"
          type="text"
          inputmode="numeric"
          placeholder="000.000.000-00"
          class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <label for="func-data" class="text-sm font-medium">Data de admissão *</label>
        <input
          id="func-data"
          v-model="dataAdmissao"
          type="date"
          required
          class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
        />
        <p class="text-xs text-neutral-400 dark:text-neutral-500">
          Usada para gerar automaticamente os períodos de férias.
        </p>
      </div>

      <div class="flex flex-col gap-1.5 sm:col-span-2">
        <label for="func-empresa" class="text-sm font-medium">Empresa *</label>
        <!-- Select com busca: digite o nome para filtrar quando houver muitas -->
        <SelectBusca
          v-model="empresaId"
          :options="opcoesEmpresas"
          input-id="func-empresa"
          placeholder="Busque pelo nome da empresa..."
        />
        <p class="text-xs text-neutral-400 dark:text-neutral-500">
          Digite parte do nome para filtrar ({{ empresas.length }} empresa(s)
          cadastrada(s)).
        </p>
      </div>

      <p
        v-if="erro"
        role="alert"
        class="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 sm:col-span-2 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
      >
        {{ erro }}
      </p>

      <p
        v-if="sucesso"
        role="status"
        class="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-sm text-emerald-700 sm:col-span-2 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300"
      >
        {{ sucesso }}
      </p>

      <div class="sm:col-span-2">
        <button
          type="submit"
          :disabled="salvando || !podeSalvar"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {{ salvando ? "Salvando..." : "Cadastrar funcionário" }}
        </button>
      </div>
    </form>
  </section>
</template>
