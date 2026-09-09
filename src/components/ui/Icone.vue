<script setup lang="ts">
/**
 * Ícones SVG do SCD (estilo traço, 24×24), agrupados num único lugar.
 *
 * Uso: <Icone nome="empresa" tamanho="lg" />
 * Para adicionar um ícone novo: inclua o nome em `IconeMenu`
 * (src/config/menu.ts) e o desenho em `CONTEUDO` abaixo.
 */
import type { IconeMenu } from "../../config/menu";

type IconeNome = IconeMenu;

const props = withDefaults(
  defineProps<{
    nome: IconeNome;
    /** Tamanho do ícone. */
    tamanho?: "xs" | "sm" | "md" | "lg";
  }>(),
  { tamanho: "md" },
);

const TAMANHOS: Record<NonNullable<typeof props.tamanho>, string> = {
  xs: "h-3.5 w-3.5",
  sm: "h-4 w-4",
  md: "h-5 w-5",
  lg: "h-6 w-6",
};

/** Desenhos (paths) de cada ícone — sem a tag <svg>, herdada do template. */
const CONTEUDO: Record<IconeNome, string> = {
  // Documento com "+": novo cadastro
  cadastro: `
    <path d="M5.5 4.5h13a1.5 1.5 0 0 1 1.5 1.5v13.5a1.5 1.5 0 0 1-1.5 1.5H5.5A1.5 1.5 0 0 1 4 19.5V6a1.5 1.5 0 0 1 1.5-1.5Z"/>
    <path d="M9.5 3v3M14.5 3v3"/>
    <path d="M12 10.5v6M9 13.5h6"/>`,
  // Prédio (empresa)
  empresa: `
    <path d="M3.75 21h16.5M4.5 3h15M5.25 3v18m13.5-18v18M9 6.75h1.5m-1.5 3h1.5m-1.5 3h1.5m3-6H15m-1.5 3H15m-1.5 3H15M9 21v-3.375c0-.621.504-1.125 1.125-1.125h3.75c.621 0 1.125.504 1.125 1.125V21"/>`,
  // Pessoa (funcionário)
  funcionario: `
    <path d="M12 12.75a3.25 3.25 0 1 0-3.25-3.25A3.25 3.25 0 0 0 12 12.75Z"/>
    <path d="M5.25 19.75a6.75 6.75 0 0 1 13.5 0"/>`,
  // Calendário com X (férias vencidas)
  "ferias-vencidas": `
    <rect x="3.75" y="5" width="16.5" height="15.25" rx="2.25"/>
    <path d="M8.25 2.75V6.5M15.75 2.75V6.5M3.75 10h16.5"/>
    <path d="m10 14 4 4m0-4-4 4"/>`,
  // Calendário com relógio (férias a vencer)
  "ferias-a-vencer": `
    <rect x="3.75" y="5" width="16.5" height="15.25" rx="2.25"/>
    <path d="M8.25 2.75V6.5M15.75 2.75V6.5M3.75 10h16.5"/>
    <circle cx="12" cy="15" r="3"/>
    <path d="M12 13.6v1.4l1 .7"/>`,
  // Pessoa com "+" (novo usuário)
  usuario: `
    <path d="M12 12.75a3.25 3.25 0 1 0-3.25-3.25A3.25 3.25 0 0 0 12 12.75Z"/>
    <path d="M5.25 19.75a6.75 6.75 0 0 1 13.5 0"/>
    <path d="M19.5 2.75v4M17.5 4.75h4"/>`,
  // Duas setas circulares (sincronização com a nuvem)
  sincronizacao: `
    <path d="M20 11.5A8.5 8.5 0 0 0 5.6 7.2L3.75 9.25M4 12.5A8.5 8.5 0 0 0 18.4 16.8l1.85-2.05"/>
    <path d="M3.75 5.5v3.75h3.75M20.25 18.5v-3.75h-3.75"/>`,
};
</script>

<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    class="shrink-0"
    :class="TAMANHOS[props.tamanho]"
    v-html="CONTEUDO[props.nome]"
  />
</template>
