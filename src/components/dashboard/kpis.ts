// Indicadores da tela inicial (dashboard): definição de cada card.
//
// Módulo PURO (sem Vue, sem Tauri) de propósito: assim as regras dos cards —
// valor, nota e PARA ONDE cada um leva — ficam cobertas por testes
// automatizados (`npm test`), inclusive a checagem de que toda seção de
// destino existe de verdade no menu (`src/config/menu.ts`).
import type { IconeMenu } from "../../config/menu";

/** Card da tela inicial. */
export interface Kpi {
  chave: string;
  rotulo: string;
  icone: IconeMenu;
  valor: number;
  nota: string;
  /** Classes de gradiente do azulejo do ícone. */
  cor: string;
  /** Seção (item do menu) aberta ao clicar no card. */
  secao: string;
}

/** Números que a tela inicial mostra. */
export interface ContagensDashboard {
  empresas: number;
  funcionarios: number;
  vencidas: number;
  aVencer: number;
  /** Empresas cadastradas no mês atual. */
  empresasNoMes: number;
}

/**
 * Monta os cards a partir das contagens. Cada card leva para a seção
 * correspondente: clicar em "Férias a vencer", por exemplo, abre a tela de
 * alertas e alarmes daquele módulo.
 */
export function montarKpis(contagens: ContagensDashboard): Kpi[] {
  const { empresas, funcionarios, vencidas, aVencer, empresasNoMes } = contagens;

  return [
    {
      chave: "empresas",
      rotulo: "Empresas",
      icone: "empresa",
      valor: empresas,
      nota: `+${empresasNoMes} no mês`,
      cor: "from-indigo-500 to-violet-600",
      secao: "cadastro-empresa",
    },
    {
      chave: "funcionarios",
      rotulo: "Funcionários",
      icone: "funcionario",
      valor: funcionarios,
      nota: "vinculados às empresas",
      cor: "from-sky-500 to-indigo-600",
      secao: "cadastro-funcionarios",
    },
    {
      chave: "ferias-vencidas",
      rotulo: "Férias vencidas",
      icone: "ferias-vencidas",
      valor: vencidas,
      nota: vencidas > 0 ? "aguardando regularização" : "em dia",
      cor: "from-amber-500 to-orange-600",
      secao: "cadastro-ferias-vencidas",
    },
    {
      chave: "ferias-a-vencer",
      rotulo: "Férias a vencer",
      icone: "ferias-a-vencer",
      valor: aVencer,
      nota: aVencer > 0 ? "dentro do prazo" : "nenhum período",
      cor: "from-emerald-500 to-teal-600",
      secao: "ferias-a-vencer",
    },
  ];
}

/**
 * Texto de acessibilidade do card ("Empresas: 7. Abrir Empresa").
 * Descreve o valor E a ação, para quem usa leitor de tela.
 */
export function rotuloAcessivel(kpi: Kpi, destino: string): string {
  return `${kpi.rotulo}: ${kpi.valor}. Abrir ${destino}`;
}
