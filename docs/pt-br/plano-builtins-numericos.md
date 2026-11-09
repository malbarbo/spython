# Plano: builtins numéricos em Rust, compilados para wasm32

Este plano muda uma decisão de 5/nov do [plano do
backend](plano-backend-wasmgc.md), a de que nenhum crate Rust é compilado
para `wasm32`. Ele foi revisto em 9/nov/2026.

## Por quê

O `float` precisa de algoritmos caros de escrever com o wasm-encoder e
fáceis de errar por um ulp: o `repr` do CPython (o decimal mais curto que
volta ao mesmo f64), o `fmod` de `//` e `%`, o `pow` de `**`, depois o
`float(str)`, o `math` e, na fatia 8, os inteiros grandes. Crates prontos
fazem isso (`ryu`, `libm`, `fast-float2`). O gleam do sgleam faz o mesmo
(`builtins-wasm/`, `link.rs`).

## As três camadas

1. **Emitido pelo wasm-encoder**, como hoje: tudo que toca valores GC ou
   a representação (`$Str`, `$Exc`, o `print`, o traceback).
2. **Builtins em Rust compilados para wasm32**: recebem e devolvem
   números, ou bytes num endereço da memória linear. Não conhecem GC,
   Python nem exceções. Uma falha volta como código de status, e o código
   emitido lança a exceção com a mensagem do CPython.
3. **Prelude em Python**, a partir da fatia 1.

Cabe na 3, vai para a 3. Precisa de valor GC, vai para a 1. A 2 é para
algoritmo numérico ou de bytes.

Restrições da 0c, que caem depois: sem alocador, sem `memory.grow`, sem
estado. Na fatia 8 o bigint precisa de uma arena com mark e release, como
no gleam. No REPL (fatia 4), os builtins vão para o módulo do runtime, com
uma memória e uma pilha só, e `builtin(name)` passa a devolver um import.

## O crate

- `compiler/builtins/`, fora do workspace (`[workspace]` próprio).
- `#![cfg_attr(target_arch = "wasm32", no_std)]` e o `#[panic_handler]`
  só em wasm32, para o `cargo test` nativo funcionar. `crate-type =
  ["cdylib", "rlib"]`. Release com `lto`, `codegen-units = 1`,
  `panic = "abort"`, `opt-level = "s"`, `strip = true`.
- Pilha de 64 KiB (`-zstack-size=65536`), com a pilha primeiro (padrão do
  wasm-ld). Um estouro sai de 0 e dá trap fora dos limites, sem
  corromper nada. Um teste soma os prólogos que descem o
  `__stack_pointer` no caminho mais fundo e afirma que cabe, o que vale
  enquanto não houver recursão.
- `--export=__heap_base`. O ligador lê esse valor e apaga o export.
- Cada export é `extern "C"` com `#[unsafe(no_mangle)]`, sobre uma função
  Rust segura que os testes nativos chamam.
- Um teste confere o `.wasm`: nenhum `unreachable` (o LTO põe o
  `panic_handler` inline, então nenhum caminho de pânico pode sobrar),
  nenhum import, tabela, elem, `__wasm_call_ctors`, `memory.grow` ou
  `memory.size`.

## O build

O `.wasm` não vai para o git. O `compiler/build.rs` chama o cargo dentro
de `compiler/builtins/`, com o target `wasm32-unknown-unknown` e um
diretório de build no `OUT_DIR`, e o compiler inclui o resultado com
`include_bytes!`. O build usa o toolchain do compiler, então todo build
precisa desse target, e os jobs do CI o instalam. O build.rs tira do
ambiente o que vale só para o build de fora, como o `--target` de um
release e o driver do clippy. Os testes do `.wasm` ficam em
`compiler/tests/builtins.rs`, sobre o binário que o build.rs gerou.

O Makefile ganha, para o crate, o `cargo test`, o `cargo clippy` e o
`cargo fmt --check`, porque os comandos do workspace não o alcançam.

## O ligador

`compiler/src/emit/builtins.rs`, com `wasm-encoder` ganhando a feature
`wasmparser` (o `reencode` fica atrás dela).

- O `.wasm` é lido uma vez (`LazyLock`).
- `ModuleState::builtin(name) -> FuncIdx` reserva um índice com a
  assinatura do export na primeira chamada e devolve o mesmo depois.
- No `finish`, busca em largura pelas instruções `call` a partir dos
  builtins usados. Um export alcançado por outro mapeia para o mesmo
  índice reservado, e as funções internas vêm depois das do programa.
  Os corpos passam pelo `Reencode`, que troca os índices de função, de
  tipo (inclusive os tipos de bloco multivalue, que vêm dos corpos) e de
  global.
- O `__stack_pointer` vira um global mutável do módulo. Os globais
  imutáveis do Rust não são copiados (só o `__heap_base` era lido, no
  carregamento).
- Os segmentos ativos do Rust entram depois dos passivos do programa, e
  os `DataIdx` do programa não mudam. O `DataCount` conta os dois. As
  seções `name`, `producers` e `target_features` são descartadas.
- O ligador confere que a pilha vem antes do primeiro segmento.

**O layout da memória é fixo**, haja builtin usado ou não, porque o
`Host::new` emite as constantes de `host.rs` antes de se saber o que o
programa usa:

```
[0, 64 KiB)            a pilha do Rust
[64 KiB, __heap_base)  os dados do Rust, inclusive o .bss
[base, base + 64 KiB)  a área do host.rs, com base = __heap_base alinhado
```

As constantes de `host.rs` viram deslocamentos sobre `base`, lido do
`.wasm` no `LazyLock`. A memória mínima cresce para caber tudo. O
`code_map` continua certo, porque relê o binário final.

## O `repr` de float

`f64_repr(f: f64, ptr: i32) -> i32` escreve o ASCII do `repr(f)` e
devolve o tamanho, no máximo 24 bytes (`-2.9205048131065683e-196`). Os
dígitos vêm de `ryu::format_finite`, reescritos no formato `'r'` do
CPython. Com `e` o expoente científico:

- notação fixa quando `-4 <= e < 16`, com `.0` se não há parte decimal;
- senão a mantissa, sem ponto quando tem um dígito só (`1e+16`), e o
  expoente com sinal e pelo menos dois dígitos (`1e-05`);
- `inf`, `-inf`, `nan` (também para `-nan`) e `-0.0`.

A revisão comparou essa conversão com o python3.13 em 3,5 milhões de
casos, sem diferença. O teste do crate usa a mesma mistura, com semente
fixa e uma chamada só ao python3: bits aleatórios, subnormais, inteiros,
decimais curtos, todas as potências de 2 e de 10, e as bordas.

O emissor escreve os bytes numa área de rascunho do host e passa cada um
ao `write_char`, para não pular o estado do `TextIOWrapper`.

## A fatia 0c, o `float`

Com o `strict_numbers`, a anotação `float` no código do aluno é só
`float`, e o `int | float` aparece só em parâmetros dos stubs e no `**`
de dois `int`. Os dois ficam fora da 0c, então a 0c não precisa da caixa
`$Float` nem de despacho.

Commits:

1. `compiler: add the builtins crate with f64_repr`: o crate, o
   build.rs, o target no CI, os alvos do Makefile, os testes do `.wasm` e
   os testes nativos contra o CPython.
2. `compiler: link the builtins that a module calls`: o ligador e o
   layout fixo. Teste de unidade que monta um módulo com `f64.const` e a
   chamada do `f64_repr`, roda no wasmtime e no deno.
3. `compiler: compile float literals and print them`: `FloatRepr::F64`,
   `emit/repr/float.rs`, `Rep::Float`, `print(2.5)`.
4. `compiler: compile + - * and unary - on float`, com a mistura com
   `int` por `f64.convert_i32_s` (exato com 31 bits), `-x`, `+x`.
5. `compiler: compile / with ZeroDivisionError`: `int / int` e os floats,
   com as mensagens `division by zero` e `float division by zero`.
6. `compiler: compile // and % on float with fmod`: o `_float_div_mod`
   do CPython, com o sinal do zero (`0.0 % -2` é `-0.0`, `-0.0 // 1` é
   `-0.0`) e as mensagens do CPython. O `fmod` é uma porta do musl,
   porque o crate `libm` e o `%` do Rust, que no wasm32 chama o `fmod`
   do `compiler_builtins`, deixam um `unreachable` no `.wasm`. O `pow`
   do commit 8 pode ter o mesmo problema.
7. `compiler: compare floats and test their truth`: comparações com
   `nan` (encadeadas também), `0.0` e `-0.0` falsos, `nan` verdadeiro.
8. `compiler: compile ** on float`: porta do `float_pow` do CPython na
   camada 2, com os casos especiais (`0.0 ** -inf` é `inf`,
   `(-1.0) ** inf` e `1.0 ** nan` são `1.0`) e uma grade testada contra
   o python3. Base negativa com expoente `float` não chega aqui, porque
   o lint `float-exponent` a recusa (decisão 11). Se um caso escapar do
   lint, ele dá o erro de limite, já que não há `complex`. Overflow dá
   `OverflowError: (34, 'Numerical result out of range')`, e
   `0.0 ** -1.0` dá `ZeroDivisionError`.

Fora da 0c: `int ** int` (o tipo é `int | float` quando o expoente não é
literal), a caixa `$Float` e o despacho de `int | float`, `float(str)`,
`math`, `round` e o hash.
