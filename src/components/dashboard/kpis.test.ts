// Testes dos cards da tela inicial: valores, notas e — principalmente — para
// onde cada card leva. Rodam com o executor do Node (`npm test`).
import { describe, test } from "node:test";
import assert from "node:assert/strict";

import { montarKpis, rotuloAcessivel } from "./kpis.ts";
import { secaoPorId, todasAsSecoes } from "../../config/menu.ts";

/** Contagens de base usadas nos testes (todas zeradas = sistema novo). */
const ZERADO = { empresas: 0, funcionarios: 0, vencidas: 0, aVencer: 0, empresasNoMes: 0 };

describe("montarKpis", () => {
  test("todo card aponta para uma seção que existe no menu", () => {
    for (const kpi of montarKpis(ZERADO)) {
      assert.ok(
        secaoPorId(kpi.secao),
        `o card "${kpi.rotulo}" aponta para a seção "${kpi.secao}", que não existe no menu`,
      );
    }
  });

  test("os destinos são exatamente as telas correspondentes", () => {
    const destinos = Object.fromEntries(montarKpis(ZERADO).map((k) => [k.chave, k.secao]));
    assert.deepEqual(destinos, {
      empresas: "cadastro-empresa",
      funcionarios: "cadastro-funcionarios",
      "ferias-vencidas": "cadastro-ferias-vencidas",
      "ferias-a-vencer": "ferias-a-vencer",
    });
  });

  test("cada destino é único (nenhum card leva à tela de outro)", () => {
    const secoes = montarKpis(ZERADO).map((k) => k.secao);
    assert.equal(new Set(secoes).size, secoes.length, "há destinos repetidos");
  });

  test("os valores vieram das contagens, na ordem esperada", () => {
    const kpis = montarKpis({
      empresas: 7,
      funcionarios: 3,
      vencidas: 12,
      aVencer: 5,
      empresasNoMes: 2,
    });
    assert.deepEqual(
      kpis.map((k) => [k.chave, k.valor]),
      [
        ["empresas", 7],
        ["funcionarios", 3],
        ["ferias-vencidas", 12],
        ["ferias-a-vencer", 5],
      ],
    );
    assert.equal(kpis[0].nota, "+2 no mês");
  });

  test("a nota das férias vencidas muda conforme há pendência", () => {
    const comPendencia = montarKpis({ ...ZERADO, vencidas: 1 });
    const semPendencia = montarKpis(ZERADO);
    assert.equal(comPendencia[2].nota, "aguardando regularização");
    assert.equal(semPendencia[2].nota, "em dia");
    assert.equal(semPendencia[3].nota, "nenhum período");
  });

  test("as chaves dos cards são estáveis (a tela usa como key)", () => {
    assert.deepEqual(
      montarKpis(ZERADO).map((k) => k.chave),
      ["empresas", "funcionarios", "ferias-vencidas", "ferias-a-vencer"],
    );
  });
});

describe("rotuloAcessivel", () => {
  test("descreve o valor e a ação do card", () => {
    const kpi = montarKpis({ ...ZERADO, empresas: 7 })[0];
    assert.equal(rotuloAcessivel(kpi, "Empresa"), "Empresas: 7. Abrir Empresa");
  });
});

describe("menu", () => {
  test("secaoPorId acha as seções do menu e devolve undefined para id inexistente", () => {
    assert.equal(secaoPorId("ferias-a-vencer")?.label, "Férias a vencer");
    assert.equal(secaoPorId("secao-que-nao-existe"), undefined);
  });

  test("todasAsSecoes reúne os itens de todos os grupos, sem repetir ids", () => {
    assert.ok(todasAsSecoes.length >= 4);
    const ids = todasAsSecoes.map((s) => s.id);
    assert.equal(new Set(ids).size, ids.length, "há ids repetidos no menu");
  });
});
