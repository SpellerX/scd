<script setup lang="ts">
// Busca de empresa por CNPJ (BrasilAPI) + gravação no banco.
// Fluxo: digita CNPJ → Buscar → confere os dados → Salvar empresa.
import { computed, ref } from "vue";
import { apenasDigitos, cnpjValido, formatarCnpj } from "../../utils/cnpj";
import { mensagemDeErro } from "../../utils/erros";
import { useEmpresas, type DadosEmpresa } from "../../composables/useEmpresas";

const { buscarPorCnpj, salvar, listar } = useEmpresas();

// Entrada com máscara: guardamos só os dígitos e exibimos formatado.
const digitos = ref("");
const cnpjExibido = computed({
  get: () => formatarCnpj(digitos.value),
  set: (valor) => {
    digitos.value = apenasDigitos(valor);
  },
});

const resultado = ref<DadosEmpresa | null>(null);
const buscando = ref(false);
const salvando = ref(false);
const erro = ref<string | null>(null);
const sucesso = ref<string | null>(null);

async function buscar() {
  erro.value = null;
  sucesso.value = null;
  resultado.value = null;

  if (!cnpjValido(digitos.value)) {
    erro.value = "Informe o CNPJ completo (14 dígitos).";
    return;
  }

  buscando.value = true;
  try {
    resultado.value = await buscarPorCnpj(digitos.value);
  } catch (err) {
    erro.value = mensagemDeErro(err);
  } finally {
    buscando.value = false;
  }
}

async function salvarEmpresa() {
  if (!resultado.value) return;

  erro.value = null;
  sucesso.value = null;
  salvando.value = true;
  try {
    // Salva com os dados já buscados (sem nova consulta à BrasilAPI).
    const empresa = await salvar(resultado.value);
    sucesso.value = `"${empresa.razao_social}" foi cadastrada com sucesso!`;
    resultado.value = null;
    digitos.value = "";
    await listar(); // atualiza a listagem abaixo
  } catch (err) {
    erro.value = mensagemDeErro(err);
  } finally {
    salvando.value = false;
  }
}

/** Exibe "—" quando o campo é opcional e está vazio. */
function ouTracinho(valor: string | null | undefined): string {
  return valor && valor.trim() !== "" ? valor : "—";
}
</script>

<template>
  <section
    class="rounded-2xl border border-neutral-200 bg-white p-5 shadow-sm dark:border-neutral-700 dark:bg-neutral-800"
  >
    <h2 class="text-sm font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
      Buscar por CNPJ
    </h2>
    <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
      Dados públicos da Receita Federal via Minha Receita (fallback: BrasilAPI).
    </p>

    <form class="mt-4 flex gap-2" @submit.prevent="buscar">
      <input
        v-model="cnpjExibido"
        type="text"
        inputmode="numeric"
        placeholder="00.000.000/0000-00"
        aria-label="CNPJ da empresa"
        class="w-full max-w-56 rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900"
      />
      <button
        type="submit"
        :disabled="buscando"
        class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {{ buscando ? "Buscando..." : "Buscar" }}
      </button>
    </form>

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

    <!-- Prévia dos dados encontrados (antes de salvar) -->
    <dl v-if="resultado" class="mt-4 grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
      <div class="sm:col-span-2">
        <dt class="text-xs uppercase text-neutral-400">Razão social</dt>
        <dd class="font-medium">{{ resultado.razao_social }}</dd>
      </div>
      <div>
        <dt class="text-xs uppercase text-neutral-400">Nome fantasia</dt>
        <dd>{{ ouTracinho(resultado.nome_fantasia) }}</dd>
      </div>
      <div>
        <dt class="text-xs uppercase text-neutral-400">Situação</dt>
        <dd>{{ ouTracinho(resultado.situacao) }}</dd>
      </div>
      <div>
        <dt class="text-xs uppercase text-neutral-400">Município / UF</dt>
        <dd>
          {{ ouTracinho(resultado.municipio) }}{{ resultado.uf ? " / " + resultado.uf : "" }}
        </dd>
      </div>
      <div>
        <dt class="text-xs uppercase text-neutral-400">Telefone</dt>
        <dd>{{ ouTracinho(resultado.telefone) }}</dd>
      </div>
      <div class="sm:col-span-2">
        <dt class="text-xs uppercase text-neutral-400">E-mail</dt>
        <dd>{{ ouTracinho(resultado.email) }}</dd>
      </div>
    </dl>

    <button
      v-if="resultado"
      type="button"
      :disabled="salvando"
      @click="salvarEmpresa"
      class="mt-4 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-60"
    >
      {{ salvando ? "Salvando..." : "Salvar empresa" }}
    </button>
  </section>
</template>
