# SCD — Sistema de Controle de Demandas

Aplicativo desktop com **Tauri v2** (Rust) + **Vue 3** (TypeScript/Vite) + **Tailwind CSS v4**.

> Foco do projeto: **código fácil de ler e alterar** — cada parte tem um lugar
> óbvio e os padrões de extensão estão comentados no próprio código.

## Funcionalidades (atual)

- **Tela de login** (página inicial): autentica no backend Rust, com opção
  **"Manter-me conectado"** — a sessão fica salva e o app abre já logado
  (token persistido na tabela `sessions`; "Sair" encerra a sessão)
- **Banco local SQLite**: usuários com senha hasheada (bcrypt)
- **Dados padronizados**: nomes de usuário em minúsculas e CNPJs sempre com os
  14 dígitos, normalizados num único ponto do backend
- **Dashboard com menu superior**: grupos **Cadastro** (Empresa,
  Funcionários, Férias vencidas, Férias a vencer) e **Sistema** (Usuários)
- **Dashboard de indicadores** (tela inicial): totais de empresas,
  funcionários, férias vencidas e a vencer + últimas empresas cadastradas
- **Cadastro de Empresa**:
  - busca de dados públicos pelo CNPJ (**Minha Receita** — API aberta, sem
    limite prático de consultas; **BrasilAPI** entra como fallback);
  - importação em lote por planilha **.xls/.xlsx**;
  - listagem das empresas cadastradas, com **exclusão individual e em lote**
    (seleção por checkbox + confirmação nativa)
- **Cadastro de Funcionários**: colaboradores **vinculados a uma empresa**
  (nome, **data de admissão obrigatória** — gera as férias automaticamente —
  e **CPF opcional** validado quando informado), com **importação em lote por
  planilha .xls/.xlsx** (colunas: nome e admissão obrigatórias; cpf opcional;
  empresa por linha via coluna `cnpj` ou empresa padrão escolhida na tela) +
  listagem/exclusão — a base para o controle de férias. Empresas com
  funcionários vinculados **não podem ser excluídas**
- **Administração de usuários** (**Sistema → Usuários**): cadastrar/excluir
  logins (nome de exibição, usuário em minúsculas e senha com mínimo de 4
  caracteres); o usuário `admin` é protegido contra exclusão
- **Acessos por usuário**: o administrador escolhe **quais módulos cada
  usuário pode ver** (menu é filtrado automaticamente no login); `admin`
  sempre tem acesso total
- **Controle de férias automático**: ao cadastrar/importar funcionário com
  data de admissão, o sistema gera os períodos (12 meses aquisitivos + 12
  para gozar). **Férias vencidas** (prazo expirado) têm botão
  **Regularizar** (já gozou, com observação). **Férias a vencer** mostram o
  prazo e permitem configurar **alerta de N dias** antes do vencimento
- **Notificações**: sino interno no cabeçalho **+ janela de notificação
  própria** (segunda janela nativa sem borda, canto superior direito, estilo
  MSN) que permanece até o usuário agir: **Regularizar agora** (abre o app na
  página de férias vencidas) ou **Lembrar mais tarde** (pausa até o dia
  seguinte). Funciona em dev e no app instalado — não usa o plugin de
  notificação do Tauri

## Estrutura do projeto

```
scd/
├── index.html              # HTML base (aplicação é montada no #app)
├── vite.config.ts          # Vite: plugins vue() + tailwindcss(), ajustes p/ Tauri
├── src/                    # ── FRONTEND (Vue 3) ─────────────────────────
│   ├── main.ts             # Bootstrap: monta o App.vue e importa o CSS
│   ├── style.css           # Entrada do Tailwind (@import "tailwindcss")
│   ├── App.vue             # Shell/raiz: tema + tela conforme autenticação
│   ├── config/
│   │   └── menu.ts         # ⭐ Itens do menu superior (edite aqui p/ mudar o menu)
│   ├── composables/        # Estado/lógica reutilizável do Vue
│   │   ├── useAuth.ts      # Sessão (login/logout/current_user)
│   │   ├── useNavigation.ts# Seção ativa do dashboard
│   │   ├── useEmpresas.ts  # Lista/estado do cadastro de empresas
│   │   └── useFuncionarios.ts # Lista/estado do cadastro de funcionários
│   ├── utils/              # Funções puras (máscaras CPF/CNPJ, erros)
│   ├── components/         # Peças de UI reutilizáveis (sem regra de negócio)
│   │   ├── LoginForm.vue
│   │   ├── navigation/TopMenu.vue   # Menu superior com submenus
│   │   ├── empresa/                 # Componentes da tela de empresa
│   │   │   ├── BuscaEmpresa.vue     # Busca por CNPJ (fontes públicas) + salvar
│   │   │   ├── ImportarEmpresa.vue  # Importação .xls/.xlsx + progresso
│   │   │   └── ListaEmpresas.vue    # Tabela + exclusão individual/em lote
│   │   └── funcionarios/            # Componentes da tela de funcionários
│   │       ├── CadastroFuncionario.vue # Formulário (CPF opcional) + vínculo
│   │       ├── ImportarFuncionarios.vue # Importação .xls/.xlsx + relatório
│   │       └── ListaFuncionarios.vue   # Listagem + exclusão
│   └── views/              # Telas do app
│       ├── LoginView.vue   # Página inicial: login
│       ├── DashboardView.vue # Área autenticada: cabeçalho + menu + conteúdo
│       └── sections/
│           ├── EmpresaView.vue      # Seção "Empresa"
│           ├── FuncionariosView.vue # Seção "Funcionários"
│           └── UsuariosView.vue     # Seção "Usuários" (Sistema)
└── src-tauri/              # ── BACKEND (Rust / Tauri) ───────────────────
    ├── tauri.conf.json     # Configuração do Tauri (janela, build, bundle)
    ├── capabilities/default.json  # Permissões (ex.: dialog de arquivos)
    └── src/
        ├── main.rs         # Entrada do binário (só chama run())
        ├── lib.rs          # Monta o app: banco, estado global e comandos
        ├── db.rs           # Camada de dados: abre o SQLite e o esquema
        ├── auth.rs         # Regras de autenticação
        ├── cnpj.rs         # Fontes públicas de CNPJ (Minha Receita/BrasilAPI) + normalização
        ├── empresas.rs     # Regras do cadastro de empresas (+ importação)
        ├── funcionarios.rs # Regras dos funcionários (vínculo com empresas)
        ├── planilha.rs     # Leitura de .xls/.xlsx (crate calamine)
        └── commands/       # Comandos Tauri expostos ao frontend
            ├── mod.rs      # Registro dos módulos de comando
            ├── auth.rs     # login, logout, current_user
            ├── empresas.rs # listar/buscar/salvar/excluir/importar empresas
            ├── funcionarios.rs # listar/criar/excluir/importar funcionários
            └── usuarios.rs # listar/criar/excluir usuários
```

## Como alterar (padrões do projeto)

**Adicionar um item ao menu superior**
1. Edite `src/config/menu.ts` (grupo + subitens);
2. Se a seção ganhar tela própria, registre-a em `telasDasSecoes` no
   `src/views/DashboardView.vue` (senão ela abre o placeholder).

**Criar uma nova tela de seção**
1. Crie `src/views/sections/MinhaSecaoView.vue`;
2. Registre em `telasDasSecoes` (DashboardView) com o `id` do item do menu.

**Criar um novo comando Rust (frontend ↔ backend)**
1. Crie `src-tauri/src/commands/<area>.rs` com `#[tauri::command]`;
2. Declare `pub mod <area>;` em `commands/mod.rs` e registre a função no
   `generate_handler!` de `lib.rs`;
3. Chame do Vue com `invoke(...)` (veja `useEmpresas.ts` como exemplo).

**Nova tabela no banco**
1. Adicione o `CREATE TABLE IF NOT EXISTS` em `src-tauri/src/db.rs`;
2. Crie as funções de acesso num módulo de regras (como `empresas.rs`).

## Planilha de importação de empresas

- Formatos: **.xls** e **.xlsx** (lidos sem depender do Excel instalado).
- A **1ª linha deve ser o cabeçalho**; o sistema procura:
  - coluna de **CNPJ**: obrigatória — cabeçalho contendo `cnpj`;
  - **razão social**: opcional — cabeçalho contendo `raz` (ex.: "Razão social");
  - **nome fantasia**: opcional — cabeçalho contendo `fantas`.
- Linhas com razão social são salvas direto; sem razão social, o sistema
  consulta as fontes públicas (Minha Receita primeiro; BrasilAPI como
  fallback, com retry automático no limite de consultas) — importações
  grandes podem demorar.
- CNPJ já cadastrado é pulado e contado como "duplicada"; linhas com problema
  aparecem no relatório final com o número da linha e o motivo.
- Dica: formate a coluna de CNPJ como **texto** na planilha, para preservar
  zeros à esquerda.

## Banco de dados

- Arquivo: `scd.db`, criado no diretório de dados do app
  (Windows: `%APPDATA%\com.scd.app\`).
- **Resetar em desenvolvimento**: feche o app e apague o arquivo — o esquema
  e o usuário demo são recriados no próximo início.
- Credencial demo: usuário `admin`, senha `admin`
  (ver `auth::ensure_demo_user` — remover quando houver cadastro real).

## Comandos úteis

```bash
npm install           # instala as dependências
npm run tauri dev     # app completo: janela nativa + backend + HMR
npm run build         # type-check (vue-tsc) + build do frontend
npm run tauri build   # binário/instalador de produção
```

## Pré-requisitos

- Node.js 18+ e Rust (stable): https://tauri.app/start/prerequisites/
- Windows: WebView2 Runtime (padrão no Win10/11)
