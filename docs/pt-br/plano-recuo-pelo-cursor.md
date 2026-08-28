# Plano: o recuo pelo cursor

## Contexto

O `repl_ready` (ENGINE.md, e o `plano-repl-continuacao.md` deste diretório) tirou
do host a pergunta "essa entrada terminou?". Ela é feita ao engine, que a lê do
parser, e por isso os dois prompts -- o do terminal e o do navegador -- leem uma
linha igual.

Onde o cursor entra, isso não vale. O `repl_ready` recebe o texto e mais nada,
então cada host decidiu por conta própria o que fazer quando o cursor não está no
fim: o `enter_action` no CLI do spython, o `AutoIndentHandler` com o
`local_indent` no do sgleam, o `enterAction` com o `newlineEdit` no SimpleCode.
Três leituras da mesma tecla.

E elas já discordam. Com

```
x = 1
y = 2
```

e o cursor no meio da segunda linha, logo depois do `y`:

| host | o que faz |
| --- | --- |
| CLI do spython | **roda as duas linhas** |
| CLI do sgleam | abre uma linha |
| SimpleCode | abre uma linha |

Não é um caso construído: é o aluno voltando para corrigir a segunda linha e
apertando Enter. No terminal o programa roda, no navegador não. O
`plano-repl-continuacao.md` foi escrito para acabar com esse tipo de diferença, e
ela sobreviveu do lado que ninguém tinha olhado.

A segunda metade do problema é o recuo. No meio da entrada o SimpleCode indenta
por regex (`indentPattern: /:$/` mais `dedentKeywords`), e o engine já lê melhor
o mesmo texto:

| texto antes do cursor | engine | regex do host |
| --- | --- | --- |
| `def f() -> int:` / `    a = 1` | 4 | 4 |
| `d = {` | 4 | 4 |
| `x = 1  # nota:` | 0 | **4** |
| `s = """` / `titulo:` | 0 | **4** |
| corpo escrito em 8, `        return a` | 0 | **4** |

## A decisão

O Enter passa a ser uma pergunta só, feita ao engine, com o texto **inteiro** e a
posição do cursor. O engine responde as duas coisas que hoje o host decide
sozinho: se a entrada roda, e com que recuo a linha abre.

Não porque a política seja gramática -- "Enter no meio de um bloco abre linha" é
convenção de editor, igual em qualquer linguagem. É porque ela tem que ser a
mesma nos três prompts, e o engine é a única coisa que os três compartilham.
Independência de linguagem diz onde o código *pode* morar; não diz que os hosts
*podem* divergir. Escrever no host é escrever três vezes e torcer -- que é
exatamente o que aconteceu.

## O que o cursor traz de novo

Além de unificar, ele dá ao engine o que ele nunca teve: **o texto depois do
cursor**.

```
def f() -> int:
    a = 1
    return a     ← cursor
    b = 2
```

Hoje os três hosts recuam a linha nova para 0, porque os três raciocinam só com o
que vem antes -- `return` fecha um bloco. Mas o bloco continua embaixo, e a linha
inserida vai ser lida ali. Com o texto todo, a linha de baixo diz isso.

Foi o argumento que quase me fez arquivar a ideia ("o engine nunca vai ver a
linha de baixo") e ele estava errado: o engine não vê porque o host corta o texto
no cursor antes de perguntar, não porque não caiba na pergunta.

## A assinatura

```
repl_ready(repl: i32, ptr: i32, len: i32, cursor: i32) -> i32
```

- `cursor` é um deslocamento em **bytes** dentro do UTF-8 em `ptr`, como o
  `repl_complete` já recebe. Um cursor fora de fronteira de caractere -- o que um
  host que conta em UTF-16 manda -- anda para a fronteira anterior. O `word_at`
  do sgleam já faz isso, pelo mesmo motivo: fatiar no cursor cru entra em pânico,
  e um pânico encerra a sessão.
- O retorno não muda: `-1` roda, `>= 0` é o recuo da linha nova, em espaços.
- O `repl` fica. Hoje nenhum dos dois engines usa (é `_repl` no spython), mas não
  custa nada e uma sessão pode vir a pesar na resposta.
- O `repl_complete` já recebe sessão e cursor. Nada a fazer nele.

### Compatibilidade

Mudar a aridade de um export existente quebra em silêncio: host novo contra
engine antiga passa um argumento a mais e a engine lê lixo como cursor.

Duas saídas:

- **(a)** nome novo (`repl_enter`), o antigo detectado como hoje
  (`!!exports.repl_ready`). Custo: dois nomes para sempre, e a pergunta antiga
  fica de pé sem ninguém para fazer.
- **(b)** trocar em cima e subir as três engines junto com o host.

**Decidido: (b).** As três engines estão no mesmo repositório --
`engines/sgleam` e `engines/spython` como submódulos, `engines/expr` em árvore
--, então "todas as engines" é uma lista fechada e curta. O `hasReady` continua
existindo para a engine que não exporta nada.

O ponto que faltava verificar eram as gravações, e ele está resolvido: o
`recording.ts` deixa o `ready-checked` de fora do formato gravado de propósito
-- "responde a uma pergunta que o replay nunca faz" --, então o replay não
chama a engine e a mudança de aridade não o alcança.

## A regra

### Roda ou abre linha

- Entrada de **uma linha**: roda se estiver terminada, onde quer que o cursor
  esteja. É a convenção de prompt de sempre, e é o que os três hosts já querem.
- Entrada de **várias linhas**: roda só com o cursor no fim.

Isso adota a regra que o sgleam e o SimpleCode já têm. **O CLI do spython muda**:
hoje ele roda com o cursor no meio da última linha, e passa a abrir linha. É a
divergência da tabela do começo, resolvida a favor dos dois.

### O recuo, quando não roda

- do **prefixo** sai qual bloco está aberto no cursor -- é o que o
  `continuation_indent` (spython) e o `nesting_depth` (sgleam) já fazem;
- do **sufixo** sai quão fundo o código desse bloco está escrito: a primeira
  linha depois da linha do cursor que não seja em branco nem comentário;
- a resposta é **o mais fundo dos dois**;
- **sem sufixo** (cursor no fim), a resposta é exatamente a de hoje. Todo teste
  atual continua valendo com `cursor = len`, e é assim que eles devem ser
  migrados: sem tocar no valor esperado.

O `max` não é um desempate arbitrário: os dois são pisos, não palpites. Uma
linha mais rasa que o código de baixo fecha um bloco que ele ainda precisa
aberto -- em Python isso é erro de sintaxe na linha seguinte. E uma linha mais
rasa que o `:` de cima não é corpo de nada. O que sobra de palpite é o dedent
do `return`, que é só uma sugestão, e por isso o sufixo o vence.

Uma versão anterior deste plano dizia "quando os dois discordam, vale o
sufixo". Ela quebra: em `def f():` / `    if x:|` / `    return 1` o sufixo
devolveria 4, onde o corpo do `if` não cabe, e em `d = {` / `    'a': 1,|` /
`}` devolveria 0. O `max` acerta os dois e todas as linhas da tabela abaixo.

Casos que os testes têm que fixar, com `|` marcando o cursor:

| entrada | esperado | por quê |
| --- | --- | --- |
| `def f() -> int:\|` | 4 | abre bloco, sem sufixo |
| `def f() -> int:\n    a = 1\|\n    b = 2` | 4 | o bloco segue embaixo |
| `def f() -> int:\n    return a\|\n    b = 2` | 4 | **novo**: o sufixo vence o `return` |
| `def f() -> int:\n    return a\|\ndef g() -> int:` | 0 | o sufixo confirma o fim |
| `def f() -> int:\n    return a\|\n\n    # nota\n    b = 2` | 4 | branco e comentário são pulados |
| `def f() -> int:\n    if x:\|\n    return 1` | 8 | o bloco recém-aberto é o piso |
| `def f() -> int:\n    x = 1\|\ny = 2` | 4 | e a linha do cursor também é |
| `d = {\n    'a': 1,\|\n}` | 4 | dentro de colchetes o `}` não puxa para fora |
| `x = 1\ny = 2\|` | -1 | cursor no fim, entrada completa |
| `x = 1\ny = \|2` | 0 | várias linhas, cursor fora do fim: abre linha |
| `x = \|1` | -1 | uma linha só, roda onde o cursor estiver |
| `x = 1  # nota:\|` | -1 | uma linha só e completa: o `:` está no comentário |
| `x = 1  # nota:\|\ny = 2` | 0 | e não abre bloco nenhum |

A fronteira entre dois blocos é o ponto que a tabela decide e que não tem
resposta gramatical: a linha vai para o bloco de cima ou para o de baixo? As
duas são válidas. O `max` decide **pelo mais fundo**, e o que importa é que a
decisão seja uma só para todos os hosts -- que é o motivo de ela estar no
engine e de a resposta ser um número, e não a estrutura em volta do cursor.
Estrutura devolveria a escolha a cada host, que é o mecanismo pelo qual eles se
afastaram.

## O trabalho

### spython -- `engine`

- `parser::ready_state(input, cursor)`; `continuation_indent` passa a olhar o
  sufixo.
- Os testes atuais migram com `cursor = input.len()`, sem mudar valor esperado.
  Os da tabela acima são novos.

Começar por aqui: o `continuation_indent` já lê a indentação *escrita* na
entrada, então a regra do sufixo encaixa sem nenhuma decisão pendente.

### spython -- `cli`

- `enter_action` vira uma chamada só: `ready_state(line, pos)`. Somem o `above`,
  o fatiamento e o `usize::try_from(...).unwrap_or(0)` -- que hoje transforma um
  `-1` em recuo 0 por acidente.
- `validate` (`cli/repl.rs:184`) chama com `cursor = input.len()`.

### sgleam -- `engine`

- `shell::ready_state(input, cursor)`, e o mesmo no `parser`.
- **Decidido: o canônico fica.** O sgleam responde `nesting_depth * INDENT`, e
  continua respondendo. É o que o CLI já faz com o cursor no fim -- o
  `local_indent` só entrava no meio --, gleam tem formatador canônico, e com a
  regra do `max` o código escrito mais fundo que o padrão é respeitado pelo
  sufixo de qualquer forma.

### sgleam -- `cli`

- `AutoIndentHandler` passa a chamar sempre; `local_indent` e seus testes
  (`repl_reader.rs:428` e `:615`) somem.
- `validate` com `cursor = input.len()`.

### wasm -- as três engines

- `engines/spython/wasm/src/lib.rs`, `engines/sgleam/wasm/src/lib.rs`,
  `engines/expr/src/lib.rs`: assinatura nova.
- `engines/sgleam/wasm/sgleam.ts:138` -- o `replReady` que quebra o stdin em
  entradas. Não tem cursor: passa `len`.

### SimpleCode

- `src/langs/config.ts:81` -- assinatura.
- `src/worker.ts:129` -- `checkReady(code, cursor)`.
- `src/repl_input.ts` -- o guard `"mid-input"` sai do `enterAction`: quem responde
  isso agora é o engine. O `readyAction` perde o `atEnd`. O `isIncomplete` fica,
  como fallback para engine sem o export.
- `src/ui.ts` -- o cursor viaja junto com o `id` que já viaja.
- `ENGINE.md` -- a seção do `repl_ready`: o parâmetro, o que o cursor significa, e
  a regra da fronteira.
- `tests/engine_test.ts` -- `readyCases` ganha cursor; os casos de hoje entram
  com `cursor = len`.

### Fora do escopo, por decisão

O **editor de arquivos** não muda. O `editor.ts:127` continua com o `newlineEdit`.
Consequência a declarar em voz alta: dentro da própria web, o Enter do editor e o
do REPL continuam podendo discordar -- `x = 1  # nota:` indenta num e não no
outro. Se isso incomodar depois, é o mesmo export, aplicado a outro teclado.

## Ordem

1. spython: engine e CLI. A regra nasce onde não há decisão pendente, e o CLI já
   valida na mão.
2. sgleam: engine e CLI, com a decisão do canônico resolvida.
3. Um commit só: wasm das três engines, `ENGINE.md`, host e testes. A aridade
   muda, e nenhum estado intermediário compila contra o outro lado.
4. Ponteiros dos submódulos no simplecode.

## O que foi feito

Passos 1 a 3 estão implementados, nos commits `8a8c811` (spython),
`9235e1de2` (sgleam) e `ebde200` (simplecode). O passo 4 -- os ponteiros dos
submódulos -- depende de as engines estarem publicadas, e nada foi enviado.

Uma coisa a mais apareceu no caminho e entrou junto: o host mandava o cursor
em unidades de JS, e a engine lê o texto em bytes. Numa linha com acentos os
dois não são o mesmo número, e o cursor chegava antes de onde estava -- um
Enter no fim de `# numero de itens` seria lido como um Enter no meio da linha.
O `worker.ts` passou a converter (`TextEncoder` sobre o texto até o cursor). O
`repl_complete` tem o mesmo problema e continua com ele: é chamada antiga, e
está fora deste plano.

## Riscos

- **Aridade.** Engine antiga com host novo lê lixo como cursor. Mitigado por
  subir junto; o que precisa de verificação antes é o replay das gravações.
- **A regra do sufixo é nova.** Não tem uso em produção que a valide; quem a fixa
  é a tabela de testes, e ela é o lugar de discutir os casos, não o código.
- **Mudança de comportamento no CLI do spython** com o cursor no meio da última
  linha. É intencional e é metade do motivo do plano, mas é visível para quem já
  usa.
