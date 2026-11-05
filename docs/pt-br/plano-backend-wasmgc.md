# Plano: o backend WasmGC

O objetivo é rodar o programa do aluno mais rápido e com um engine menor.
O caminho é compilar o Python do spython para WasmGC em vez de
interpretá-lo no RustPython. O RustPython sai quando o compilador cobrir o
corpus. Até lá os dois convivem, e o backend é escolhido por sessão.

O compilador fica num crate novo, `compiler/`, no `main`. Ele entra atrás
da opção `backend=wasmgc` (`--backend=wasmgc` na CLI, `backend=wasmgc` no
`repl_new`), desligada por padrão. Os branches `origin/wasm-compiler` e
`origin/compiler-wasm-gc-standalone` não voltam. Deles ficam os programas
de teste e o gerador de fixtures sobre o `na-programacao`.


## Por que recomeçar

Os dois branches são protótipos de abril, sobre o fork `spython-0.1`, com
cerca de 4 mil linhas cada.

O `origin/wasm-compiler` lê os tipos pela sintaxe das anotações e guarda
`int` num `i64` com wrap. Nele, `fat(21) > fat(20)` dá falso, `-7 // 2`
dá `-3`, `s[0] == 'a'` gera wasm inválido, `[1, 2] == [1, 2]` compara
referências, e um método sobrescrito chama o da classe base. Também não há
`print`, exceção nem linha de erro.

O `origin/compiler-wasm-gc-standalone` usa o `ty`, mas decide a
representação comparando o tipo impresso (`"list[int]"`) e cai numa
inferência própria quando o `ty` diz `Unknown`. Cobre os níveis 0 e 1 e
`list[int]`, sem laços nem classes.

Nenhum dos dois tem a representação uniforme que herança, uniões,
`isinstance` e exceções pedem. Pôr essa representação neles é reescrever
tudo. Três revisões adversariais (técnica, de escopo e de risco)
confirmaram que recomeçar sai mais barato.

Os modelos vêm de três projetos vizinhos. Do jwonder vêm os objetos: o
campo de classe na posição 0, os campos herdados como prefixo, as vtables
como structs imutáveis de `funcref` e o cuidado com o colapso de tipos de
mesma forma. Do backend Gleam vêm os erros a evitar, segundo a revisão
dele: identidade pelo nome impresso, um gerador com dezenas de campos, um
arquivo de 6 mil linhas e decisões repetidas em vários lugares. Do sjava
vem o processo, com as regras escritas antes do código e o plano revisado.
O contrato com o host é o da seção "Modules" do `ENGINE.md` do simplecode,
e o loader nativo segue o `Runtime` do sc.


## Decisões

Estas decisões foram tomadas em 5/nov/2026, uma pergunta por vez.

1. O objetivo é velocidade e tamanho. O RustPython sai no fim.
2. O fork do ruff ganha um arquivo, `ty_python_semantic/src/types/lowering.rs`,
   com as consultas de que o compilador precisa, e o gancho do lint do
   `float`. Nada além disso muda no fork.
3. O nível 5 não precisa ser coberto inteiro. O RustPython sai quando o
   corpus passar: o `na-programacao`, os exemplos da documentação e a lib
   `spython`. O que o compilador não cobrir vira diagnóstico do checker.
   Não há fallback automático para o RustPython.
4. Onde a anotação diz `float`, o valor tem de ser `float`. O checker
   rejeita `massa: float = 7874` e pede `7874.0`. Com isso, `float` é
   sempre `f64`.
5. A CLI embute o wasmtime, atrás de uma feature, desde a fatia 0a. O
   `fd_write` e o `fd_read` são escritos à mão, sem `wasmtime-wasi`.
6. Um commit que só toca `compiler/` passa pelo portão rápido: `make check`
   e `cargo test -p compiler`, com o diferencial do corpus. Os outros rodam
   todas as suítes.
7. `int` é `i31ref`. Os inteiros grandes ficam para depois. Até lá, uma
   operação cujo resultado não cabe em 31 bits lança `OverflowError`, e
   nunca dá wrap.


## Desenho

### O caminho do código

```
fonte → ty → lowering → IR tipada → emissão (wasm-encoder) → validação (wasmparser)
```

O engine já roda o `ty` antes de executar. O lowering lê o AST e as
consultas do `ty` e produz uma IR estruturada, em árvore, porque o
controle de fluxo do Python já é estruturado. Cada nó da IR tem um tipo
de representação, e as coerções, as chamadas ao runtime e o despacho
aparecem nela de forma explícita. Toda decisão de semântica do Python fica
no lowering. A emissão só traduz.

Uma função, `rep`, decide a representação de um tipo, e outra, `coerce`,
decide a conversão entre duas representações. Nenhum outro lugar decide
isso. O estado do módulo e o estado da função ficam em structs separadas,
e os locais são alocados sob demanda. A emissão é determinística, e não
há `wasm-opt`.

### As consultas ao `ty`

O `lowering.rs` fica em `types/`, para enxergar os itens `pub(super)` de
lá, e devolve um enum do spython, `Shape`. Fora dele, o fork só ganha a
linha `pub mod lowering;`. A sondagem pediu oito consultas:

- `expr_shape`, o tipo de uma expressão depois do estreitamento;
- `declared_shape`, o tipo de uma atribuição anotada;
- `slot_shape`, o tipo declarado do lugar que uma atribuição preenche;
- `function_info`, os parâmetros e o retorno;
- `class_info`, a MRO, os métodos e o que cada um sobrescreve, os
  atributos, os campos da dataclass, os membros do Enum e o
  `__match_args__`;
- `call_target`, se a chamada vai a uma função, a um método ou a um
  construtor;
- `call_bindings`, o parâmetro de cada argumento, com o tipo
  especializado;
- `attribute_owner`, a classe da MRO que define um atributo.

Classes e funções são identificadas pelo arquivo e pelo intervalo da
definição, nunca pelo nome impresso. As variáveis capturadas por closures
vêm da API pública do `ty`, sem mudança no fork.

O `ty` estreita o tipo de uma variável pela atribuição. Depois de
`q: Ponto = p`, ele diz que `q` é `Ponto3`. A representação e o despacho
usam o tipo declarado, por `slot_shape`, e a chamada de método passa pela
vtable.

O `ty` lê a anotação `float` como `int | float`. O gancho no fork desliga
isso no código do aluno, e mantém nos stubs só nas anotações de parâmetro.
Assim o próprio `ty` rejeita `7874`, `sum(xs)` e `math.floor(x)` onde se
espera `float`, e aceita `math.sqrt(2)` e `2.0 + 1`. No `na-programacao`,
o único diagnóstico novo é o de `massa_tubo_ferro.py`. O gancho vira uma
opção do `ty` e pede um ajuste nas mensagens, que hoje mostram `float*`.

### Valores

A forma uniforme é `(ref null eq)`, e `None` é `null`.

`int` é `i31ref`. As operações têm o caminho rápido inline e testam o
transbordo, que lança `OverflowError`. A divisão inteira e o resto seguem
o Python (`-7 // 2 == -4`).

`float` é `f64` em locais, parâmetros e campos, e vai numa caixa `$Float`
quando passa pela forma uniforme. `bool` é `i32` em locais, e na forma
uniforme é um dos dois singletons de `$Bool`, subclasse de `int`.

`str` guarda um code point por elemento, num `(array i32)`, com índice
O(1). A conversão para UTF-8 só acontece na fronteira com o host. Os
caracteres isolados vêm de uma tabela, para `s[i]` não alocar.

Todo valor do heap descende de `$Object {class}`. Uma classe do usuário é
um subtipo com os campos herdados como prefixo. A `$Class` guarda o nome,
a profundidade e os ancestrais, e a vtable é uma struct imutável de
`funcref`. `isinstance` e `except` comparam a `$Class`. Um `ref.test`
contra um tipo do usuário nunca decide `isinstance`, porque tipos de
mesma forma colapsam num só tipo wasm.

Os tipos do runtime ficam num único rec group, copiado byte a byte em todo
módulo. Os hosts recusam o link quando a cópia difere, então o rec group
fica congelado por versão do prelude.

Uma variável capturada e reatribuída fica numa célula. Uma closure é um
par `$closure` com um `$code` por aridade, e o código é um campo imutável.

### Exceções e traceback

Uma tag só carrega a exceção, `$Exc {class, msg, tb, ctx}`. O erro sempre
sai de uma chamada a `$raise_*`.

O traceback é montado no desenrolar, sem offsets do host. Cada função
atualiza um local `$line` antes do que pode lançar. O corpo fica num
`try_table` com `catch_ref`, e o handler acrescenta um `$Frame {func, line}`
à frente do traceback e relança com `throw_ref`. O `except` do Python
testa a classe e relança quando ela não casa. Quando casa, acrescenta o
quadro da própria função, como o CPython. Um global `$handling` dá o
"During handling of the above exception", e só as funções com `except`
salvam e restauram esse global. Uma função que não pode lançar fica sem
`try_table`.

A sondagem imprimiu o mesmo traceback do spython, byte a byte, no deno e
no wasmtime. O host lê a exceção por exports que devolvem números, e
nenhum valor GC atravessa a fronteira como número. Cada módulo exporta a
tabela com os nomes das funções.

### Recursão e Stop

Um estouro real da pilha não é capturável pelo wasm. Por isso um contador
global de profundidade lança `RecursionError` a 1000 quadros. Ele é
testado na entrada da função, antes do `try_table`, e zerado a cada
`wasm_call`. Uma função folha, que não chama outra função Python, fica sem
contador.

O Stop usa a mesma comparação. O host zera o limite, e o caminho lento
confere a interrupção e dá trap por `unreachable`, que atravessa os
`except`. Os laços também testam o limite na cabeça. Na CLI, o
`max_wasm_stack` fica em 4 MiB. Se mesmo assim a pilha estourar, o host
traduz o trap ou o `RangeError` num `RecursionError` sem traceback.

As chamadas entre funções Python não usam `return_call`, porque o
traceback precisa dos quadros.

### Módulos e REPL

Cada módulo Python vira um módulo wasm, que importa o runtime e os módulos
vizinhos. O runtime e o prelude são instanciados uma vez por sessão e
importados por handle.

No REPL, cada entrada vira um módulo novo. Os dados ficam em slots
uniformes guardados no runtime. As funções de módulo ficam numa tabela de
`funcref` do runtime, e toda chamada a uma delas passa por
`call_indirect`. Redefinir uma função troca só a entrada dela na tabela.
É a busca tardia do Python. Com ela, `h = g` guardado antes de redefinir
`f` faz `h()` chamar o `f` novo, e cada entrada compila só o próprio
código. Uma classe redefinida é uma classe nova, e os objetos antigos
continuam usáveis pela vtable deles. O checker do REPL rejeita a
redefinição de uma função com outra assinatura.

O `Store` do wasmtime não libera instâncias, então na CLI a sessão do REPL
cresce cerca de 70 KB por entrada. O coletor DRC também não coleta ciclos.
Os dois limites vão para a documentação.

### Doctests

O engine já escreve uma função sintética por docstring. O compilador
compila essa função com pontos de verificação e com a saída capturada. O
resultado de cada exemplo tem de ser igual ao do runner atual.

### Os hosts

No navegador e no deno, o módulo roda pelo `wasm_instantiate` e pelo
`wasm_call` do `ENGINE.md`. O V8 compila de forma preguiçosa, e o custo de
compilação some. Lá o que importa é o tamanho do download.

Na CLI, o wasmtime 48 roda com as features `runtime`, `std`, `cranelift`,
`gc`, `gc-drc` e `parallel-compilation`, e o `cranelift_opt_level` fica em
Speed. O prelude é compilado na primeira execução e guardado como
`.cwasm` em `config_dir()/spython/`, com a chave de
`Engine::precompile_compatibility_hash`. Sem esse cache, compilar o
prelude levou 0,2 s em 28 núcleos e 1,9 s num núcleo só.

O `prelude.wasm` fica no repositório, com um teste que confere que ele
está atual, e não é gerado por `build.rs`. Um `build.rs` que compila para
`wasm32` quebraria a matriz da CI e poria o `ty` como build-dependency.

### Regras de emissão

O Cranelift tem tempo superlinear em funções com muitas alocações. Por
isso as caixas são alocadas por chamada a um helper, e a inicialização de
um módulo é partida em funções de no máximo 64 alocações. A sondagem
mediu 400 alocações inline numa função em 5,4 s, e as mesmas 400 como
chamadas em 0,06 s.


## Testes

O oráculo dos valores é o CPython. O do stderr e dos doctests é o spython
atual, sobre o RustPython. O stdout, o stderr e o resultado dos doctests
têm de ser iguais byte a byte, com uma diferença conhecida: o RustPython
deixa um espaço no fim da mensagem de recursão.

O corpus é o `na-programacao`, os programas dos branches antigos, o
`cli/tests/inputs`, os exemplos da documentação e as sondagens. As
fixtures ficam num módulo só, atrás de um dispatcher, para o teste ser
rápido, e rodam no wasmtime da CLI e no deno.


## Fatias

A ordem segue o que o prelude precisa, e os níveis ordenam só o portão do
corpus.

- **−1. Sondagens.** Feitas, com o resultado na seção seguinte.
- **0a. CLI.** `def`, `int`, `if`, `while`, `print` e a exceção não
  capturada com linha, rodando no wasmtime embutido.
- **0b. Navegador.** O mesmo no deno, com o diferencial contra o
  RustPython.
- **1. Prelude.** Arrays, classes sem herança, a `$Class` e os métodos
  especiais. Depois o prelude de `str` e de `int`, e os doctests.
- **2. Níveis 0 e 1**, com o diferencial do corpus verde.
- **3. Nível 2.** Enum, dataclass e `match` com todos os padrões.
- **4. REPL na CLI.**
- **5. Nível 3.** `list`, `for`, `range`, fatias e `+=`.
- **6. Nível 4**, com o REPL no navegador. Classes com herança, `dict`,
  `set`, compreensões, lambda e closures. É a fatia de maior risco.
- **7. Imagens e programas interativos.**
- **8. Nível 5 por partes**, até o corpus passar, e a saída do RustPython.

A estimativa é de 5 a 8 meses de trabalho ativo até a fatia 6.


## O que as sondagens mediram

As medidas vêm de código escrito à mão ou gerado por script, não do
compilador, e vão ser refeitas na fatia 0.

| Caso | wasmtime | V8 | spython `main` | python3 |
|---|---:|---:|---:|---:|
| soma de `int` abaixo de 2^30 | 0,32 s | 0,31 s | 2,72 s | 1,33 s |
| fib(30) | 0,06 s | 0,05 s | 0,79 s | 0,19 s |
| 10^7 chamadas de método | 1,11 s | 0,65 s | 7,33 s | 2,64 s |
| 10^7 chamadas de closure | 0,69 s | 0,35 s | 4,69 s | 1,41 s |
| aloca e descarta 10^7 objetos | 2,32 s | 0,37 s | 8,79 s | 2,28 s |
| árvore com ponteiro para o pai | 2,85 s | 2,99 s | 263 s | 35,3 s |

O coletor DRC do wasmtime custa cerca de 180 ns por objeto que nasce e
morre, 6 a 7 vezes o V8, e aumentar o heap inicial não ajuda. Uma
chamada por `call_ref` custa 23 ns no wasmtime e 1 ns no V8, e é o caso
da vtable e da closure. Uma chamada por `call_indirect` custa quase o
mesmo que a direta nos dois hosts.

O traceback completo, com o contador, o `$line` e o `try_table`, custa
entre 1,7 e 2,8 ns por chamada. No V8 o `try_table` não custa nada, e a
maior parte do custo é o contador.

Com o prelude e o programa em `.cwasm`, o processo inteiro levou 2,8 ms. O
Cranelift compila o programa do aluno a uns 0,25 ms por linha num núcleo.
O hello world leva hoje 141 ms no `main`, com o checker.


## Em aberto

- Os inteiros grandes: `$Long`, `$Big` e um `int` local em `i64`. Uma
  soma que sai de 31 bits e aloca uma caixa por iteração levou 2,36 s no
  wasmtime, contra 2,04 s no RustPython.
- O despacho direto quando a classe do receptor é conhecida, por causa do
  custo do `call_ref` no wasmtime.
- O que mostrar no Safari, que não dá offsets na pilha. O traceback
  montado no desenrolar não depende deles, mas o trap de Stop e o estouro
  de pilha dependem do host.
- As imagens: chamar o Image em Rust do sinteract pela fronteira ou
  reescrever a biblioteca no prelude.
