O playground web permite usar o spython diretamente no navegador, sem instalar nada.


# Layout

A interface é dividida em dois painéis:

- **Painel do editor** (esquerda ou topo): onde você escreve o código Python
- **Painel do REPL** (direita ou embaixo): onde a saída é exibida

O layout inicial é escolhido automaticamente com base nas dimensões da tela: horizontal para telas largas e vertical para telas altas.


# Barra de ferramentas

- **Run** (▶): Formata e executa as definições
- **Stop** (■): Interrompe a execução
- **Tema** (☀): Alterna entre os temas claro e escuro
- **Layout**: Alterna entre layout horizontal e vertical
- **Nível**: Seleciona o nível de ensino (0-5)


# Atalhos de teclado

| Atalho | Descrição |
|--------|-----------|
| `Ctrl+r` | Executa as definições |
| `Ctrl+f` | Formata o código |
| `Ctrl+j` | Foca no painel do editor |
| `Ctrl+k` | Foca no painel do REPL |
| `Ctrl+d` | Mostra/esconde o painel do editor |
| `Ctrl+i` | Mostra/esconde o painel do REPL |
| `Ctrl+l` | Alterna entre layout horizontal e vertical |
| `Ctrl+t` | Alterna entre tema claro e escuro |
| `Ctrl+?` | Mostra a janela de ajuda |
| `Esc` | Fecha a janela de ajuda |


# Como usar

1. Escreva suas definições no painel do editor
2. Pressione `Ctrl+r` ou clique em **Run**
3. Use o REPL para avaliar expressões usando as definições

O botão **Run** (ou `Ctrl+r`) formata o código, verifica os tipos e anotações, executa os doctests e carrega as definições no REPL. Depois disso, você pode chamar as funções definidas no editor diretamente no REPL.

O REPL verifica tipos e anotações a cada entrada. Definições sem anotação ou com construções proibidas para o nível selecionado são rejeitadas antes da execução.


# Níveis de ensino

O seletor de nível na barra de ferramentas controla quais construções de Python são permitidas:

| Nível | Nome      | Construções adicionadas                                              |
|-------|-----------|----------------------------------------------------------------------|
| 0     | Funções   | `def`, `return`, escalares, indexação de `str`                       |
| 1     | Seleção   | `if`/`elif`/`else`                                                   |
| 2     | Tipos de usuário | `class` (Enum / `@dataclass`), `match`                               |
| 3     | Repetição | literais de `list`, `for`, `while`, `+=`                             |
| 4     | Classes   | `class` com métodos, `dict`/`set`, compreensões, `lambda`            |
| 5     | Completo  | irrestrito (apenas anotações são exigidas)                           |

Os níveis 0 a 3 aplicam restrições extras focadas em erros comuns de
iniciantes, todas liberadas no nível 4:

- **Condições booleanas** — o teste de `if`/`elif`/`while`/ternário/`assert`
  e os operandos de `and`/`or`/`not` precisam ter tipo `bool`.
- **Sem `bool` em aritmética** — valores `bool` não são aceitos em `+`, `-`,
  `*`, `/`, `//`, `%`, `**`, atribuição aumentada ou unário `+`/`-`.
- **Sem comparações encadeadas** — `a < b < c` precisa ser desmembrada com
  `and`.
- **Sem expressão como statement** — uma expressão cujo resultado é
  descartado (que não seja chamada de função, docstring ou `...`) é
  rejeitada.
- **Sem valores padrão em parâmetros** — parâmetros de função não podem ter
  valores padrão.


# Temas

O playground suporta dois temas baseados no editor Zed:

- **One Light** -- tema claro (padrão)
- **One Dark** -- tema escuro

A preferência é salva no navegador.
