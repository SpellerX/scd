<script setup lang="ts">
// Seção "Empresa" (menu Cadastro): busca por CNPJ, importação por planilha
// e listagem das empresas cadastradas.
import { onMounted } from "vue";
import { useEmpresas } from "../../composables/useEmpresas";
import BuscaEmpresa from "../../components/empresa/BuscaEmpresa.vue";
import ImportarEmpresa from "../../components/empresa/ImportarEmpresa.vue";
import ListaEmpresas from "../../components/empresa/ListaEmpresas.vue";

const { listar } = useEmpresas();

// Carrega as empresas ao abrir a seção.
onMounted(() => {
  void listar();
});
</script>

<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold">Cadastro de Empresa</h1>
      <p class="mt-1 text-neutral-500 dark:text-neutral-400">
        Cadastre empresas pelo CNPJ (BrasilAPI) ou em lote por planilha.
      </p>
    </div>

    <div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
      <BuscaEmpresa />
      <ImportarEmpresa />
    </div>

    <ListaEmpresas />
  </div>
</template>
