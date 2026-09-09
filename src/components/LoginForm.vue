<script setup lang="ts">
// Formulário de login: cuida dos campos, da chamada ao backend (useAuth)
// e da exibição de erros. Sem regra de negócio — só apresentação/eventos.
import { ref } from "vue";
import { useAuth } from "../composables/useAuth";

const { login, loading, error } = useAuth();

const username = ref("");
const password = ref("");
/** "Manter-me conectado" — ligado por padrão para não pedir login toda vez. */
const lembrar = ref(true);

async function onSubmit() {
  await login(username.value, password.value, lembrar.value);
}
</script>

<template>
  <form class="flex flex-col gap-4" @submit.prevent="onSubmit">
    <div class="flex flex-col gap-1.5 text-left">
      <label for="login-username" class="text-sm font-medium">Usuário</label>
      <input
        id="login-username"
        v-model="username"
        type="text"
        name="username"
        autocomplete="username"
        autocapitalize="none"
        autocorrect="off"
        spellcheck="false"
        placeholder="Seu usuário"
        required
        class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
      />
    </div>

    <div class="flex flex-col gap-1.5 text-left">
      <label for="login-password" class="text-sm font-medium">Senha</label>
      <input
        id="login-password"
        v-model="password"
        type="password"
        name="password"
        autocomplete="current-password"
        placeholder="Sua senha"
        required
        class="rounded-lg border border-neutral-300 bg-white px-3.5 py-2 text-sm outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-indigo-500 dark:border-neutral-600 dark:bg-neutral-900 dark:placeholder:text-neutral-500"
      />
    </div>

    <!-- Erro vindo do backend (usuário/senha inválidos etc.) -->
    <p
      v-if="error"
      role="alert"
      class="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
    >
      {{ error }}
    </p>

    <!-- "Lembrar" da sessão -->
    <label
      class="flex cursor-pointer items-center justify-center gap-2 text-sm text-neutral-600 dark:text-neutral-300"
    >
      <input v-model="lembrar" type="checkbox" class="h-4 w-4 accent-indigo-600" />
      Manter-me conectado neste computador
    </label>

    <button
      type="submit"
      :disabled="loading"
      class="rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow transition hover:bg-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500 disabled:cursor-not-allowed disabled:opacity-60"
    >
      {{ loading ? "Entrando..." : "Entrar" }}
    </button>
  </form>
</template>
