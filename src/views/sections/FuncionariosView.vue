<script setup lang="ts">
// Seção "Funcionários" (menu Cadastro): cadastro (com vínculo à empresa),
// importação por planilha e listagem dos funcionários.
import { onMounted } from "vue";
import { useEmpresas } from "../../composables/useEmpresas";
import { useFuncionarios } from "../../composables/useFuncionarios";
import CadastroFuncionario from "../../components/funcionarios/CadastroFuncionario.vue";
import ImportarFuncionarios from "../../components/funcionarios/ImportarFuncionarios.vue";
import ListaFuncionarios from "../../components/funcionarios/ListaFuncionarios.vue";

const { listar: listarEmpresas } = useEmpresas();
const { listar: listarFuncionarios } = useFuncionarios();

// Carrega funcionários e empresas (para o vínculo nos formulários).
onMounted(() => {
  void listarFuncionarios();
  void listarEmpresas();
});
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Cadastro de Funcionários</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        Cadastre colaboradores vinculados a uma empresa ou importe por planilha
        — a base para o controle de férias.
      </p>
    </div>

    <div class="grid grid-cols-1 items-start gap-6 xl:grid-cols-2">
      <CadastroFuncionario />
      <ImportarFuncionarios />
    </div>

    <ListaFuncionarios />
  </div>
</template>
