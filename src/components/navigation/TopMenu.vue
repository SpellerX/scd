<script setup lang="ts">
// Menu superior: desenha os grupos do src/config/menu.ts, com ícone no
// botão do grupo e nos itens do submenu (dropdown).
// Cada grupo é um botão que abre seu submenu ao clicar; clicar fora ou
// escolher um item fecha o submenu.
import { ref } from "vue";
import type { MenuGroup, MenuItem } from "../../config/menu";
import { useNavigation } from "../../composables/useNavigation";
import Icone from "../ui/Icone.vue";

defineProps<{ groups: MenuGroup[] }>();

const { activeSection, openSection } = useNavigation();

/** Índice do grupo com o dropdown aberto (null = nenhum). */
const expandedGroup = ref<number | null>(null);

function toggleGroup(index: number) {
  expandedGroup.value = expandedGroup.value === index ? null : index;
}

function selectItem(item: MenuItem) {
  openSection(item);
  expandedGroup.value = null;
}

function isActive(item: MenuItem) {
  return activeSection.value?.id === item.id;
}
</script>

<template>
  <nav class="flex items-center gap-1">
    <template v-for="(group, groupIndex) in groups" :key="group.label">
      <div class="relative">
        <!-- Botão do grupo (ex.: Cadastro), com ícone -->
        <button
          type="button"
          :aria-haspopup="true"
          :aria-expanded="expandedGroup === groupIndex"
          @click="toggleGroup(groupIndex)"
          class="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium text-neutral-600 transition hover:bg-neutral-100 hover:text-neutral-900 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 dark:text-neutral-300 dark:hover:bg-neutral-800 dark:hover:text-white"
        >
          <Icone
            v-if="group.icon"
            :nome="group.icon"
            tamanho="sm"
            class="text-neutral-400 dark:text-neutral-500"
          />
          {{ group.label }}
          <!-- Seta indicando dropdown (gira quando aberto) -->
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            stroke-width="2"
            stroke="currentColor"
            class="h-3.5 w-3.5 transition-transform"
            :class="expandedGroup === groupIndex ? 'rotate-180' : ''"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5" />
          </svg>
        </button>

        <!-- Camada invisível que fecha o dropdown ao clicar fora dele -->
        <div
          v-if="expandedGroup === groupIndex"
          class="fixed inset-0 z-10"
          @click="expandedGroup = null"
        />

        <!-- Submenu (dropdown) com ícones nos itens -->
        <ul
          v-if="expandedGroup === groupIndex"
          role="menu"
          class="absolute left-0 top-full z-20 mt-1.5 w-80 rounded-xl border border-neutral-200 bg-white p-1.5 shadow-lg dark:border-neutral-700 dark:bg-neutral-800"
        >
          <li v-for="item in group.items" :key="item.id">
            <button
              type="button"
              role="menuitem"
              @click="selectItem(item)"
              class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition focus:outline-none"
              :class="
                isActive(item)
                  ? 'bg-indigo-50 font-medium text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300'
                  : 'text-neutral-700 hover:bg-neutral-100 dark:text-neutral-200 dark:hover:bg-neutral-700'
              "
            >
              <!-- Ícone do item dentro de um mini azulejo -->
              <span
                v-if="item.icon"
                class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg transition"
                :class="
                  isActive(item)
                    ? 'bg-indigo-100 text-indigo-600 dark:bg-indigo-900 dark:text-indigo-300'
                    : 'bg-neutral-100 text-neutral-500 dark:bg-neutral-700 dark:text-neutral-300'
                "
              >
                <Icone :nome="item.icon" tamanho="sm" />
              </span>
              {{ item.label }}
            </button>
          </li>
        </ul>
      </div>
    </template>
  </nav>
</template>
