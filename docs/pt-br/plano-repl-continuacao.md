# Plano: continuação de leitura no REPL pelo parser

## Contexto

No sgleam (commits `c8f306c30`, `95a88e6c6`, `368380600`) a decisão "essa entrada
está terminada ou o prompt continua?" deixou de olhar só o *tipo* do erro de
parse e passou a olhar também a *posição* dele: `let x =` e `let x = )` produzem
o mesmo `ExpectedValue` sobre o mesmo `=`, mas no primeiro não sobrou token
nenhum depois do erro — foi isso que `ends_there` passou a perguntar.

No spython a decisão está em `cli/repl.rs`, no `Validator` do rustyline
(`ReplHelper::validate`), e hoje é feita em duas partes:

1. `is_incomplete_error` — lista de tipos de erro (`Lexical(Eof)`,
   `UnclosedStringError`, `FStringError(_)`) mais um casamento de texto:
   `msg.starts_with("Expected an indented block")`.
2. `input_has_block` — heurística de string: "alguma linha termina em `:`".

A entrada é compilada com `vm.compile(Mode::Single)`, que por baixo chama
`ruff_python_parser::parse`. Ou seja: o parser já está no caminho, mas o que
chega ao REPL é um `CompileError` que guarda apenas `SourceLocation`
(linha/coluna do *início*) — a posição em bytes e os demais erros se perdem.

## O que o parser do ruff informa

Medido diretamente contra `ruff_python_parser::parse_unchecked` (que devolve
todos os erros e o fluxo de tokens, em vez do primeiro erro):

| entrada | primeiro erro | intervalo |
| --- | --- | --- |
| `x = ` | `ExpectedExpression` | `4..4` (vazio, no fim) |
| `def f():` | `OtherError("Expected an indented block after ...")` | `8..8` |
| `@dataclass` | `OtherError("Expected class, function definition ... after decorator")` | `10..10` |
| `x = [1,` | `Lexical(Eof)` | `7..7` |
| `x = )` | `ExpectedExpression` | `4..5` (sobre o `)`) |
| `x = 1 1` | `SimpleStatementsOnSameLine` | `6..7` |
| `)` | `OtherError("Expected a statement")` | `0..1` |

O padrão é limpo: **entrada inacabada → o primeiro erro é vazio e cai onde a
entrada parou; entrada acabada e errada → o primeiro erro cobre um token real**.
Duas diferenças em relação ao gleam:

- o ruff aponta o erro sobre o *token ofensor*, não sobre o token anterior, então
  o teste tem que usar `location.start()` (com `end()`, `x = )` seria lido como
  inacabado e o prompt travaria);
- o parser do ruff é *recuperador*: depois de um token ruim ele emite um segundo
  erro vazio no fim ("Expected a statement"). Por isso a decisão tem que olhar o
  **primeiro** erro, não todos.

Como no sgleam, comentários e quebras de linha não contam como "algo depois do
erro" — `def f():` seguido de uma linha só com comentário reporta o erro em
`8..9`, longe do fim, e sem filtrar os tokens de trivia o prompt encerraria ali.

## Quanto isso rende em Python

Diferente do gleam, em Python uma expressão não continua na linha seguinte: `1 +`
mais uma linha nova continua sendo erro de sintaxe. O ganho da regra por posição
é, por isso, bem menor aqui — sobre todos os prefixos de linha dos `.py` do repo
(`lib/`, `examples/`, `cli/tests/`) que o parser rejeita, 2620 de 2622 (99,9%) já
são lidos como inacabados hoje. Os dois que faltam, porém, importam:

- `@dataclass` sozinho — hoje o REPL responde com erro de sintaxe em vez de pedir
  a próxima linha. É o construto central do nível 2.
- um bloco cuja primeira linha depois do cabeçalho é só um comentário.

Além desses, a regra por posição corrige de graça `x = a.`, `def`, `class`,
`with`, `x = lambda` e — se o erro de continuação de linha for tratado — `x = 1 + \`.
E remove dois *traps* atuais, em que o prompt fica pedindo linha para uma entrada
que nunca vai fechar: `f"{x!q}"` e `f"{x:"`, hoje aceitos pelo casamento amplo
`FStringError(_)`.

## A regra, como ficou

`engine/src/parser.rs::is_incomplete` continua a leitura só onde uma linha nova
de fato resolve. Em Python isso é: delimitador aberto, string de três aspas
aberta, barra invertida no fim, bloco esperando corpo indentado e decorador
esperando `def`/`class`.

```rust
pub fn is_incomplete(src: &str) -> bool {
    let parsed = parse_unchecked_source(src, PySourceType::Python);
    let Some(err) = parsed.errors().first() else {
        // Parseia, e um bloco segue até uma linha em branco fechá-lo.
        return parsed.syntax().body.last().is_some_and(is_compound_statement)
            && !ends_with_blank_line(src);
    };
    match &err.error {
        ParseErrorType::Lexical(LexicalErrorType::Eof) => true,
        ParseErrorType::Lexical(LexicalErrorType::UnclosedStringError) => {
            is_triple_quoted(&src[err.location])
        }
        ParseErrorType::Lexical(
            LexicalErrorType::FStringError(InterpolatedStringErrorType::UnterminatedTripleQuotedString)
            | LexicalErrorType::TStringError(InterpolatedStringErrorType::UnterminatedTripleQuotedString),
        ) => true,
        ParseErrorType::Lexical(LexicalErrorType::LineContinuationError) => {
            err.location.end() >= end_of_text(src)
        }
        ParseErrorType::OtherError(_) => {
            ends_there(&parsed, err.location.start()) && !ends_with_blank_line(src)
        }
        _ => false,
    }
}
```

Três pontos que a medição contra o parser da versão fixada no `Cargo.lock`
mudou em relação ao esboço inicial:

- uma string de uma aspa só sem fechar dá o mesmo `UnclosedStringError` que uma
  de três — o que separa as duas é o texto sobre o qual o erro é reportado, que
  começa na abertura da string, prefixo `rb` e tudo. Só a de três aspas continua
  na linha seguinte;
- o mesmo vale para as f-strings, onde só `UnterminatedTripleQuotedString`
  continua. `f"{x` e `f"{x!q}"` são erros na hora, como no CPython;
- uma linha em branco encerra o que esperava corpo — sem isso, `def f():` e
  Enter duas vezes prenderia o prompt sem saída a não ser Ctrl-C. Dentro de um
  delimitador ou de uma string de três aspas a linha em branco é conteúdo, e por
  isso a escapatória vale só para o ramo `OtherError`.

A alternativa mais fiel ao sgleam — aplicar a posição a *todo* tipo de erro —
também foi medida (100% dos prefixos), mas em Python ela leva a `x = `, `1 +`,
`assert`, `import ` e `del` continuarem o prompt sem que a linha seguinte possa
completá-los: o usuário digita, leva o erro de sintaxe do mesmo jeito e ainda
gasta uma linha. O CPython também erra na hora nesses casos.

## O fim de bloco, e o Enter

`input_has_block` ("alguma linha termina em `:`") era independente do acima e
tinha erros próprios: `x = 1  # nota:` e `s = """\ntitulo:\n"""` parseiam limpo
e pediam um Enter extra. Agora a pergunta é feita à árvore: o último `stmt` de
nível superior é composto? Se for, e a entrada não terminar em linha em branco,
continua — que é o que o CPython faz.

O `SmartKeys` (o handler de Enter) decidia por conta própria, com as mesmas
heurísticas de string, e era ele quem via a tecla *antes* do validador. Os dois
agora chamam a mesma função, e o indent da próxima linha vem de
`continuation_indent`, que lê o `:` do fluxo de tokens — um `:` dentro de
comentário ou de string não indenta mais nada.

## Entrada pendente no fim do stdin

Com o prompt continuando em mais casos, ficou visível o buraco que o sgleam
fechou no commit `368380600`: quando o stdin acaba no meio de uma entrada, o
editor a descarta e não diz nada. O validador agora guarda o que ficou
pendente e o laço do REPL o executa ao receber `Eof` — o erro é do usuário para
ler. Um Ctrl-C descarta o pendente, que foi abandonado e não está esperando.

## A pergunta do Enter, e o `repl_ready`

O SimpleCode passou a perguntar a mesma coisa ao engine, pelo export opcional
`repl_ready(repl, ptr, len) -> i32` (`ENGINE.md`, commit `2108136`). Antes o
prompt do navegador adivinhava o fim de bloco contando delimitadores e casando
os padrões de indentação da config da linguagem — a gramática escrita uma
segunda vez no host, e errada onde as duas leituras discordam.

A resposta é `-1` para rodar a entrada, ou a indentação com que a *próxima*
linha começa, em espaços. `-1` também é a resposta para o que não parseia de
jeito nenhum: o usuário vê o erro do engine em vez de um prompt que não o
solta. E se a linguagem fecha bloco com linha em branco, é o engine que diz
isso — o host não fecha nada.

Do lado do spython isso é `engine::parser::ready_state`, que é o
`is_incomplete` já existente mais o `continuation_indent`, e o export
`repl_ready` em `wasm/src/lib.rs` que o chama. A sessão não é consultada: o que
uma entrada Python está esperando está no texto dela, e o parser lê isso sem
interpretador rodando. O leitor do terminal (validador + Enter do `SmartKeys`)
chama a mesma função, então os dois prompts leem uma linha igual.

O recuo ganhou quatro regras para isso, porque agora ele é resposta de API e
não só um detalhe do rustyline:

- um delimitador aberto vale um nível, quantos forem abertos de uma vez —
  `print(f(` é uma linha para indentar, não duas. Dentro deles a linha segue o
  texto acima, que é onde o `:` de um dicionário não indenta nada;
- fora deles, quem manda é a indentação da linha *lógica*, não a da última
  linha física. Depois de `print(\n    1\n)` dentro de um bloco, a próxima
  linha volta ao nível do bloco, e um cabeçalho escrito em várias linhas abre
  o bloco a partir de onde o cabeçalho começa;
- um comentário não é uma linha lógica: a linha em que ele está continua sendo
  a de cima. Depois de `def f() -> int:` e `    # soma`, o bloco abre em 4 —
  não em 8, somando um nível sobre uma indentação que já era a do bloco;
- o que fecha um bloco (`return`, `pass`, ...) volta para onde o bloco de fora
  está escrito, lido da entrada, em vez de tirar um nível fixo. Com o corpo em
  8, tirar 4 pediria uma coluna onde não há bloco nenhum, e o Python recusa.

Quem pergunta pelo terminal ainda escolhe *de onde* pergunta (`enter_action`):
no fim da entrada, Enter roda o que está escrito, esteja o cursor onde estiver
na última linha — como em qualquer editor de linha. Acima dela, Enter abre uma
linha e não roda nada, com o recuo que o texto até o cursor pede: é o bloco em
que o cursor está, não aquele em que a entrada termina.

## O que mudou

- `engine/src/parser.rs` (novo) — `ready_state` é a superfície; `is_incomplete`
  e `python_part` são internos. A entrada é lida uma vez: `ready_state` parseia
  e passa os tokens adiante, e um único caminhar sobre eles (`layout`) responde
  onde cada comando começa, quantos delimitadores estão abertos e qual foi o
  último token. Os testes não precisam de uma `VirtualMachine`, e um deles
  cobre todos os prefixos de linha de um programa válido.
- O comando é tirado sem lista de comandos: o que vem depois do primeiro espaço
  é lido como Python (`:time [1,` continua, `:quit` não tem o que continuar).
  Uma lista aqui envelheceria junto com a do REPL sem que nada quebrasse.
- `engine/src/completion.rs` — `expected_indent` (o `ends_with(':')`) saiu; o
  Tab agora indenta pelo `continuation_indent`, então Tab e Enter respondem à
  mesma pergunta do mesmo jeito. `INDENT` é público e o Backspace do CLI o usa.
- `engine/Cargo.toml` — `ruff_python_parser` deixa de ser dev-dependency. Sem
  impacto em tamanho de binário: `ty_python_semantic` já o linka.
- `cli/repl.rs` — `validate` e o Enter do `SmartKeys` (por `enter_action`)
  chamam `ready_state`;
  saem `is_incomplete_error`, `input_has_block`, `is_dedent_keyword` e a
  compilação extra por Enter. O comando é tirado antes de perguntar, no engine
  (`:time [1,` agora continua), e o pendente no fim do stdin é executado.
- `cli/Cargo.toml` — a dependência `rustpython-compiler`, que só existia para os
  tipos de erro do validador, saiu.
- `wasm/src/lib.rs` — o export `repl_ready`, com teste em
  `wasm/tests/wasm_test.ts` contra o binário WASM de verdade.

## Aberto

- `enter_action` (de onde a pergunta é feita, dado o cursor) está no CLI. O
  protocolo do `repl_ready` não passa cursor, então o editor do navegador vai
  precisar da mesma regra um dia; quando precisar, ela sobe para o engine.
