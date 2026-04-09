# Compilador Python -> wasm-gc

## Objetivo

Compilar um subconjunto tipado de Python para `wasm-gc`, substituindo o
RustPython como backend de execucao.

## Estado atual

O compilador atual ja existe como o crate [compiler/Cargo.toml](/home/malbarbo/projetos/spython/compiler/Cargo.toml) e gera `wasm` diretamente com `walrus`.

O modulo gerado hoje e:

- `standalone`
- sem imports de host
- executavel diretamente pelo `wasmtime`
- baseado em `wasm-gc`

O frontend valida o programa com `ty` antes do codegen.

## Representacao atual

Nao existe representacao uniforme por `anyref`. O backend atual usa valores wasm tipados:

- `int` -> `i64`
- `float` -> `f64`
- `bool` -> `i32`
- `str` -> referencia GC para array de bytes ASCII
- `list[int]` -> referencia GC para array de `i64`

### Strings

`str` hoje e um `array` GC de bytes ASCII.

- literais sao internados como data segments
- o modulo constroi a string com `array.new_data`
- operacoes sao implementadas por helpers internos em wasm
- literais nao ASCII geram erro de compilacao

Semantica atualmente suportada:

- `+`
- `*` com `int`
- `==`
- `len`
- indexacao
- slicing sem `step`
- `.upper()`
- `.lower()`
- `str(int)`

### Listas

`list[int]` hoje e um `array` GC de `i64`.

Semantica atualmente suportada:

- literal de lista
- `==`
- `len`
- indexacao
- slicing sem `step`
- atribuicao em indice

Ainda nao ha representacao `Vec-like` com `len/capacity`, nem listas genericas.

## Subconjunto suportado hoje

### Modulo

- `def`
- globais tipadas
- `assert` no topo
- atribuicoes simples no topo

O export principal e `run() -> i32`.

- `0` significa sucesso
- valor diferente de `0` indica o indice do `assert` que falhou

### Funcoes

- chamadas diretas
- chamadas para funcoes definidas depois
- recursao direta
- `return`
- variaveis locais tipadas

### Controle de fluxo

- `if`
- `elif`
- `else`
- expressao ternaria

### Escalares

- aritmetica de `int` e `float`
- comparacoes
- `and`, `or`, `not`
- widen `int -> float` quando necessario

Helpers numericos implementados no proprio modulo:

- `//` e `%` com semantica de Python para `int`
- `abs`
- `min`
- `max`
- `round`
- `math.ceil`
- `float ** int`

## O que ainda nao esta pronto

- Unicode em `str`
- `print`, `input` e runtime de IO
- `for`, `while`, `range`
- `list[T]` generica
- `tuple`
- `dict`
- classes, `@dataclass`, `Enum`, heranca
- `match/case`
- closures

## Testes end-to-end

Existe um gerador de fixtures em [compiler/src/bin/compiler-testset.rs](/home/malbarbo/projetos/spython/compiler/src/bin/compiler-testset.rs) e os testes materializados vivem em [compiler/tests/generated](/home/malbarbo/projetos/spython/compiler/tests/generated).

O teste end-to-end principal esta em [compiler/tests/e2e.rs](/home/malbarbo/projetos/spython/compiler/tests/e2e.rs) e:

- compila Python para `wasm`
- instancia o modulo com `wasmtime`
- executa `run`
- falha se algum `assert` falhar

Estado atual do corpus:

- fixtures ASCII gerados de `02`, `03` e `04` passam no fluxo end-to-end
- a expansao para `09-recursividade` ainda e exploratoria e nao faz parte do conjunto obrigatorio
- ha teste explicito para rejeicao de string nao ASCII
- ha teste explicito para `//` e `%` com operandos negativos

## Estrutura do backend

O codegen ainda e direto para wasm, mas ja existe um pequeno "micro-lowering"
local em pontos de maior pressao semantica:

- chamadas
- subscript

Isso reduz a quantidade de decisao semantica feita no meio da emissao.

O backend usa agora a API fluida do `walrus` em [compiler/src/compile.rs](/home/malbarbo/projetos/spython/compiler/src/compile.rs); os usos manuais de `dangling_instr_seq` e `.instr(...)` foram removidos do crate `compiler`.

## Proximos passos provaveis

1. Aumentar o conjunto obrigatorio de fixtures que passa em `wasmtime`.
2. Cobrir mais casos reais de `09-recursividade`.
3. Estender o runtime de colecoes alem de `list[int]`.
4. Decidir se a proxima etapa de compartilhamento semantico entre backends sera por micro-lowering adicional ou por uma IR pequena propria.
