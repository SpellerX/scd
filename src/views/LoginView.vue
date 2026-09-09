<script setup lang="ts">
// Tela de login — a página inicial do SCD.
// Só monta a apresentação (cartão com a marca) e delega o formulário
// ao componente LoginForm.
import LoginForm from "../components/LoginForm.vue";
</script>

<template>
  <main class="flex flex-1 items-center justify-center p-6">
    <div
      class="w-full max-w-sm rounded-2xl bg-white p-8 text-center shadow-lg ring-1 ring-neutral-200 dark:bg-neutral-800 dark:ring-neutral-700"
    >
      <!-- Marca do sistema -->
      <div
        class="mx-auto flex h-14 w-14 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-600 to-violet-600 text-lg font-bold tracking-wider text-white shadow"
      >
        SCD
      </div>
      <h1 class="mt-4 text-xl font-bold">Sistema de Controle de Demandas</h1>
      <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
        Entre para acessar o sistema
      </p>

      <div class="mt-6">
        <LoginForm />
      </div>

      <!--
        Ajuda de desenvolvimento: mostra a credencial criada automaticamente
        pelo backend (auth::ensure_demo_user). Remova quando houver cadastro real.
      -->
      <p class="mt-6 text-xs text-neutral-400 dark:text-neutral-500">
        Demo: usuário <code class="font-semibold">admin</code> · senha
        <code class="font-semibold">admin</code>
      </p>
    </div>
  </main>
</template>
