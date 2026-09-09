<script setup lang="ts">
// Seção "Usuários" (menu Sistema): cadastro, acessos e listagem dos usuários
// que podem acessar o SCD.
import { onMounted } from "vue";
import { useUsuarios } from "../../composables/useUsuarios";
import CadastroUsuario from "../../components/usuarios/CadastroUsuario.vue";
import EditarAcessos from "../../components/usuarios/EditarAcessos.vue";
import ListaUsuarios from "../../components/usuarios/ListaUsuarios.vue";

const { listar } = useUsuarios();

onMounted(() => {
  void listar();
});
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Usuários do SCD</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        Cadastre quem pode acessar o sistema e escolha os módulos de cada um.
        O usuário <code class="font-semibold">admin</code> é o administrador
        padrão (acesso total) e não pode ser excluído.
      </p>
    </div>

    <div class="grid grid-cols-1 items-start gap-6 xl:grid-cols-2">
      <CadastroUsuario />
      <EditarAcessos />
    </div>

    <ListaUsuarios />
  </div>
</template>
