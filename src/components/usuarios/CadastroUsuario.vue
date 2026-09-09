<script setup lang="ts">
// Formulário de cadastro de usuário (login da pessoa).
import { computed, ref } from "vue";
import { mensagemDeErro } from "../../utils/erros";
import { useUsuarios } from "../../composables/useUsuarios";

const { criar, listar } = useUsuarios();

const nomeExibicao = ref("");
const username = ref("");
const senha = ref("");
const confirmaSenha = ref("");

const salvando = ref(false);
const erro = ref<string | null>(null);
const sucesso = ref<string | null>(null);

/** Usuário (login): aceita nome e sobrenome; o SCD salva em minúsculas. */
const usernameExibido = computed({
  get: () => username.value,
  set: (valor) => {
    username.value = valor.toLowerCase();
  },
});

const podeSalvar = computed(
  () =>
    nomeExibicao.value.trim() !== "" &&
    username.value.trim() !== "" &&
    senha.value.length >= 4 &&
    senha.value === confirmaSenha.value,
);

async function salvar() {
  erro.value = null;
  sucesso.value = null;

  if (senha.value.length < 4) {
    erro.value = "A senha deve ter pelo menos 4 caracteres.";
    return;
  }
  if (senha.value !== confirmaSenha.value) {
    erro.value = "A confirmação de senha não confere.";
    return;
  }
  if (!username.value.trim() || !nomeExibicao.value.trim()) {
    erro.value = "Preencha o nome de exibição e o nome de usuário.";
    return;
  }

  salvando.value = true;
  try {
    await criar({
      username: username.value.trim(),
      // Chaves do invoke em camelCase (padrão Tauri → snake_case no Rust).
      nomeExibicao: nomeExibicao.value.trim(),
      senha: senha.value,
    });
    sucesso.value = `Usuário "${username.value}" cadastrado! Agora defina os acessos dele na seção "Acessos por usuário".`;
    nomeExibicao.value = "";
    username.value = "";
    senha.value = "";
    confirmaSenha.value = "";
    await listar();
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
      Cadastrar usuário
    </h2>
    <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
      Cria um login para outra pessoa acessar o SCD.
    </p>

    <form class="mt-4 flex flex-col gap-4" @submit.prevent="salvar">
      <div class="flex flex-col gap-1.5">
        <label for="user-nome" class="text-sm font-medium">Nome de exibição *</label>
        <input
          id="user-nome"
          v-model="nomeExibicao"
          type="text"
          placeholder="Ex.: Maria Silva"
          required
          class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <label for="user-login" class="text-sm font-medium">Usuário (login) *</label>
        <input
          id="user-login"
          v-model="usernameExibido"
          type="text"
          autocomplete="off"
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
          placeholder="maria"
          required
          class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
        />
        <p class="text-xs text-neutral-400 dark:text-neutral-500">
          Pode ser nome e sobrenome (ex.: "maria silva"); será salvo em
          minúsculas, sem espaços repetidos.
        </p>
      </div>

      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div class="flex flex-col gap-1.5">
          <label for="user-senha" class="text-sm font-medium">Senha *</label>
          <input
            id="user-senha"
            v-model="senha"
            type="password"
            autocomplete="new-password"
            placeholder="Mínimo 4 caracteres"
            required
            class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <label for="user-senha2" class="text-sm font-medium">Confirmar senha *</label>
          <input
            id="user-senha2"
            v-model="confirmaSenha"
            type="password"
            autocomplete="new-password"
            placeholder="Repita a senha"
            required
            class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
          />
        </div>
      </div>

      <p
        v-if="erro"
        role="alert"
        class="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
      >
        {{ erro }}
      </p>
      <p
        v-if="sucesso"
        role="status"
        class="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-sm text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300"
      >
        {{ sucesso }}
      </p>

      <div>
        <button
          type="submit"
          :disabled="salvando || !podeSalvar"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {{ salvando ? "Salvando..." : "Cadastrar usuário" }}
        </button>
      </div>
    </form>
  </section>
</template>
