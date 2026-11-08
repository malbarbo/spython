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
2. O fork do ruff ganha um arquivo,
   `ty_python_semantic/src/types/lowering.rs`, com as consultas de que o
   compilador precisa, e os ganchos das decisões 4, 10 e 11. O engine lê do
   fork o `KnownFunction`, com o `pow`, e o `Type::as_int_literal` para os
   seus lints. Nada além disso muda no fork.
3. O nível 5 não precisa ser coberto inteiro. O RustPython sai quando o
   corpus passar: o `na-programacao`, os exemplos da documentação e a lib
   `spython`. O que o compilador não cobrir vira diagnóstico do checker.
   Não há fallback automático para o RustPython.
4. Onde a anotação diz `float`, o valor tem de ser `float`. O checker
   rejeita `massa: float = 7874` e pede `7874.0`. Com isso, `float` é
   sempre `f64`.
5. Até a última fatia, a CLI e os testes rodam o módulo no wasmtime
   externo, por `wasmtime run`, e o módulo importa do WASI só o
   `fd_write`, o `fd_read` e o `proc_exit`. No fim, a CLI embute o
   wasmtime, atrás de uma feature, e escreve essas funções à mão, sem
   `wasmtime-wasi`.
6. Um commit que só toca `compiler/` passa pelo portão rápido: `make check`
   e `cargo test -p compiler`, com o diferencial do corpus. Os outros rodam
   todas as suítes.
7. `int` é `i31ref`. Os inteiros grandes ficam para depois. Até lá, uma
   operação cujo resultado não cabe em 31 bits nunca dá wrap. Ela para o
   programa com um erro que o `except` não captura, com o traceback, o
   valor verdadeiro e o limite. Um `OverflowError` seria engolido por um
   `except Exception` e confundido com o do Python (`10.0 ** 400`).
8. O REPL fica para depois. Até ele entrar, o programa inteiro, com o
   runtime, vira um módulo wasm só, executado de uma vez.
9. A representação de `int`, `float` e `str` é extensível, como o
   `StringType` do backend Gleam, e escolhida por opção de compilação.
   Hoje há uma de cada: `i31ref`, `f64` e UCS-4. Toda decisão de
   representação fica no módulo `emit/repr/` do compilador, e nenhum
   outro arquivo a conhece.
10. `bool` não é atribuível a `int` nem a `float`, em todos os níveis.
    Sem isso, `def g() -> int: return 3 > 2` imprimiria `1` no lugar de
    `True`. No corpus, a regra não rejeita nenhum programa. Um operador de
    inteiro (`+ - * / // % ** << >>` e o `+`/`-` unário) lê um operando
    `bool` como `int`, e quem decide se a conta vale é o lint
    `bool-in-arithmetic`, que a rejeita nos níveis 0 a 3. Nenhum desses
    operadores devolve `bool`, então nenhum valor de tipo `int` é um
    `bool`.
11. `int ** int` é `int` quando o expoente é um literal ≥ 0 ou um `bool`,
    `float` quando é um literal negativo, e `int | float` quando é outro
    `int`, porque `2 ** -1` é `0.5`. `float ** int` e `** float` são
    `float`. O `ty` dava `Any` nesses casos, e o gancho fixa o tipo, também
    no `pow` com dois argumentos. Um erro de tipo sobre um desses
    `int | float` traz uma nota que sugere `int(...)` ou `float(...)`. O
    lint `float-exponent` rejeita, em todos os níveis, um expoente `float`
    com base que pode ser negativa, porque o Python daria `complex`. Ele
    aceita uma base literal positiva e um expoente literal `float` de valor
    inteiro, exceto a base 0 e o expoente `-0.0`, em que o RustPython
    diverge do CPython. No compilador, `int | float` é uma união na forma
    `(ref null eq)`, e o expoente negativo deixa de parar o programa. O
    `complex` ainda existe no spython, pelo tipo `complex` e pelos literais
    como `2j`, e pela decisão 3 o compilador pede um diagnóstico do checker
    para ele.
12. O lint `is-comparison`, em todos os níveis, rejeita `is` e `is not`
    quando um dos lados pode ser `int`, `float`, `complex`, `bool`, `str`,
    `bytes` ou `tuple`, ou é `Any` ou um `TypeVar`, a não ser que o outro
    lado seja `None`. Com esses tipos, o resultado de `is` depende da
    representação. Com `n = 1000`, `n is 1000` já dava `True` no CPython e
    `False` no RustPython. Membros de Enum, instâncias de classe, `list`,
    `dict` e `set` continuam aceitos, e o compilador os compara com
    `ref.eq`.
13. As mensagens de erro e o traceback seguem o CPython, sempre.
14. O corpus da decisão 3 inclui as transcrições de REPL dos `.md`. Por
    causa do `2 ** 80` de uma aula, o RustPython só sai depois dos
    inteiros grandes.
15. O compilador nunca vê um tipo dinâmico. Com `Any`, ele não sabe a
    representação do valor nem qual operação emitir. Despachar todo operador
    em tempo de execução custaria caro, e testar o tipo na conversão pararia
    o programa onde o CPython segue. O lint `dynamic-type`, em todos os
    níveis, rejeita um valor cujo tipo é `Any` ou `Unknown`, ou tem um deles
    dentro, como o `list[Unknown]` de um `[]` sem contexto. O erro aparece
    onde o tipo nasce: numa expressão cujas partes têm tipo conhecido, numa
    anotação, num parâmetro de lambda ou num nome importado de uma
    biblioteca. Quando há outro erro, os do `dynamic-type` somem, porque um
    nome que não resolve espalha `Unknown` por tudo que o usa. O resultado
    de `eval`, `getattr` e `open` é dinâmico, e o lint o rejeita. No corpus,
    a regra rejeita `lst == []`, em quatro exemplos de recursividade, e o
    `>>> lst = []` de um doctest. Se o compilador encontrar um tipo
    dinâmico, é bug do checker, e ele para com um erro interno.


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

O lowering dá a cada valor uma forma neutra, `Rep`, que não sabe nada de
wasm: `Int`, `Float`, `Bool`, `Str`, `None`, `Instance(ClassId)` ou
`Uniform`. Só a emissão consulta a fachada `Reprs`, que diz como cada
`Rep` e cada operação viram wasm. O estado do módulo e o estado da função ficam em structs separadas,
e os locais são alocados sob demanda. A emissão é determinística, e não
há `wasm-opt`.

### A organização do código

O `lower/` tem um arquivo por assunto: os escopos e o fluxo, as leituras
e escritas de nomes, o `NameError`, as chamadas, os comandos e as
expressões. As classes e o `match` ganham arquivos próprios quando
entrarem, `lower/class.rs` e `lower/pattern.rs`. O `cpython/` tem as regras
do CPython que o compilador calcula em Rust: os nomes que ele conhece, a
sugestão do `NameError`, as linhas do traceback, as mensagens e os
limites. As regras da organização ficam no cabeçalho do `lib.rs`.

A partir da fatia 1, o código vai para o prelude em Python, a menos que
precise de instruções wasm. O jwonder passou o runtime escrito à mão para
Java e o reduziu de 628 para 298 linhas, e o backend Gleam terminou com
três camadas de runtime e builtins repetidos.

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
o único diagnóstico novo é o de `massa_tubo_ferro.py`. O gancho é a opção
`ProgramSettings.strict_numbers` do fork, que também tira o `float*` das
mensagens. A mesma opção liga as regras de `bool` e de `**` das decisões
10 e 11. O lint de `is` fica no engine, porque só precisa do tipo dos
operandos.

### Valores

`Reprs` é o único item de `compiler/src/emit/repr/` que o emissor usa.
Os enums `IntRepr`, `FloatRepr` e `StrRepr` são privados ao módulo, então
um `match` sobre eles fora dali não compila. Um `clippy.toml` com
`disallowed-methods` proíbe fora de `emit/repr/` as instruções de
representação, como `i31.get_s`, `ref.i31` e `array.new_data`.

```
compiler/src/emit/repr/
  mod.rs      Reprs, a fachada
  uniform.rs  vt_of, eq, hash, format e truthy, que passam por todos os tipos
  object.rs   $Object, $Class, $VT, $Bool e $Undef
  int.rs      enum IntRepr { I31 }
  float.rs    enum FloatRepr { F64 }
  numeric.rs  as pontes entre int e float
  str.rs      enum StrRepr { Ucs4 }
compiler/src/emit/runtime/
  exc.rs      $Exc, $Frame, a profundidade das chamadas e o traceback
  host.rs     a saída e o fim do programa, pelo host
```

O layout das classes do aluno, com os structs, os slots da vtable e os
globais de `$Class`, sai da tabela de classes da IR e fica em
`emit/layout.rs`, como no jwonder. O significado de cada slot é decidido
no lowering, e o número dele na emissão.

Uma representação nova de `int`, de `float` ou de `str` muda só o arquivo
do tipo, e também `numeric.rs` quando o tipo é `int` ou `float`.

| Tipo do `ty` | Forma local | Forma uniforme |
|---|---|---|
| `int` | `i32`, só valores em 31 bits | `i31ref` |
| `float` | `f64` | `$Float` |
| `bool` | `i32` | um dos dois singletons de `$Bool` |
| `str` | `(ref $Str)` | igual |
| `None` | nenhuma | `null` |
| classe `C` | `(ref $C)` | igual |
| `T \| None`, com `T` não numérico | `(ref null $T)` | igual |
| o resto | `(ref null eq)` | igual |

O wasm só aceita a leitura de um local não anulável dentro do bloco que o
atribuiu. Por isso o local é guardado anulável e lido com
`ref.as_non_null`. As variáveis globais do módulo ficam em globais do wasm
na forma local, e as de uma função em locais do wasm. Quando o `ty` não
prova que uma leitura vem depois da atribuição, a variável ganha uma flag,
e a leitura testa a flag e lança `NameError` ou `UnboundLocalError`, como
no CPython. Dentro de uma função, a leitura de uma global e a chamada de
outra função sempre testam a flag, porque a função pode rodar antes da
atribuição ou do `def`.

A verdade de um valor é decidida pelo tipo estático: um `int` é
verdadeiro quando não é 0, e um `str` quando não é vazio. `and` e `or`
com operandos do mesmo tipo devolvem um dos operandos. Numa condição e
no operando de `not`, só a verdade importa, e os operandos podem ter
tipos diferentes. O despacho pela vtable só entra com as uniões e com o
`__bool__` e o `__len__` das classes.

`+` e `-` de `int` rodam em `i32` e testam os 31 bits. `*` roda em `i64`,
porque `65536 * 65536` dá 0 em `i32` e passaria no teste. `-x`, `abs` e
`//` testam o caso de −2^30. `<<`, `**`, `int()`, `round`, `sum`, `len`,
`hash` e as funções de `math` que devolvem `int` passam pelo mesmo teste.
Um literal que não cabe em 31 bits é diagnóstico do checker. A divisão
inteira e o resto seguem o Python (`-7 // 2 == -4`). No corpus, nenhum
`int` sai de 31 bits fora do `2 ** 80` de uma aula, do `2 ** 100000` da
documentação, de `count_range1.py` e do `now_ms()` da lib `spython`, que
passa de 2^30 com 12,4 dias de uptime e vai contar a partir do início do
processo.

`str` guarda um code point por elemento, num `(array i32)`, com índice
O(1). A conversão para UTF-8 só acontece na fronteira com o host, por
`str_to_host`, que recebe o host porque uma representação futura sobre as
strings do JS não tem bytes. Os caracteres isolados vêm de uma tabela,
para `s[i]` não alocar.

Os tipos do runtime ficam num único rec group.

```wasm
(rec
  (type $Class (struct
    (field $name (ref $Raw)) (field $module (ref $Raw))
    (field $depth i32) (field $ancestors (ref $Classes))))
  (type $Classes (array (ref $Class)))
  (type $VT (sub (struct
    (field $class (ref $Class))
    (field $repr (ref $Repr)) (field $str (ref $Repr))
    (field $eq (ref $Eq)) (field $hash (ref $Hash)))))
  (type $Repr (func (param (ref null eq)) (result (ref $Str))))
  (type $Eq (func (param (ref null eq) (ref null eq)) (result i32)))
  (type $Hash (func (param (ref null eq)) (result i64)))
  (type $Object (sub (struct (field $vt (ref $VT)))))
  ;; $Str e $Raw vêm de StrRepr, e $Float vem de FloatRepr
  (type $Bool (sub final $Object (struct (field $vt (ref $VT)) (field $value i32))))
  (type $Undef (sub final $Object (struct (field $vt (ref $VT)))))
  (type $Values (array (mut (ref null eq))))
  (type $Exc (sub $Object (struct
    (field $vt (ref $VT)) (field $args (ref $Values))
    (field $tb (mut (ref null $Frame))) (field $ctx (mut (ref null $Exc))))))
  (type $Frame (struct (field $site i32)
                       (field $next (ref null $Frame)))))
```

Todo valor do heap descende de `$Object`, com a vtable no campo 0. Uma
classe do usuário é um subtipo com os campos herdados como prefixo, e a
vtable é uma struct imutável de `funcref`. `$Class` não tem vtable nem
`$Str`, e assim os tipos não formam ciclo. O nome é o array cru, e
`__name__` cria a `$Str` quando alguém pede. `ancestors` guarda só os
ancestrais estritos, e `isinstance(x, K)`, com `c` a classe de `x`, é
`c == K || (c.depth > K.depth && c.ancestors[K.depth] == K)`. Um
`ref.test` contra um tipo do usuário nunca decide `isinstance`, porque
tipos de mesma forma colapsam num só tipo wasm.

`vt_of` devolve a vtable de qualquer valor uniforme. `null` dá a de
`NoneType`, um `i31ref` dá a de `int`, um `$Object` dá o próprio campo, e
uma `$Class` dá a de `type`. `type`, `isinstance`, `except`, `print`,
`dict` e `set` passam por ela. `eq` e `hash` tratam a torre numérica antes
da vtable, e `{1: 'a', True: 'b', 1.0: 'c'}` é `{1: 'c'}`. O hash dos
números é o do CPython, módulo 2^61 − 1, inclusive o de `float`, porque a
ordem de um `set` aparece no `print`. `format` e as f-strings passam por
`int.__format__` e `float.__format__`.

Uma variável capturada e reatribuída fica numa célula. Uma closure é um
par `$closure` com um `$code` por aridade, e o código é um campo imutável.

### Exceções e traceback

Uma tag só carrega a exceção, `$Exc {vt, args, tb, ctx}`. A classe
calcula `str(e)` a partir de `args`. O erro sempre sai de uma chamada a
`$raise_*`, e as mensagens e o formato do traceback são os do CPython.

O traceback é montado no desenrolar, sem offsets do host. Um site é um
lugar do programa que pode lançar, e o compilador guarda o texto do quadro
de cada site, com o arquivo, a linha, a função, a linha do código e as
marcas `~~^^` do CPython 3.13. Cada função atualiza um local `$site` antes
do que pode lançar. O corpo fica num `try_table`, e o handler acrescenta
um `$Frame {site}` à frente do traceback e lança de novo. Cada site também
tem uma chave de linha, o par da função e do número da linha, e o
traceback escreve os três primeiros quadros de uma sequência com a mesma
chave e depois "[Previous line repeated N more times]", como o
`StackSummary.format` do CPython. O `except` do Python
testa a classe e relança quando ela não casa. Quando casa, acrescenta o
quadro da própria função, como o CPython. Um global `$handling` dá o
"During handling of the above exception", e só as funções com `except`
salvam e restauram esse global. Uma função que não pode lançar fica sem
`try_table`.

A sondagem imprimiu o mesmo traceback do spython de hoje, byte a byte, no deno e
no wasmtime. Com o wasmtime externo, o próprio módulo escreve o
traceback no fd 2 e sai com `proc_exit(1)`. Quando o host for nosso, ele
vai ler a exceção por exports que devolvem números, e nenhum valor GC
vai atravessar a fronteira como número. Cada módulo exporta a tabela com
os nomes das funções.

### Recursão e Stop

Um estouro real da pilha não é capturável pelo wasm. Por isso um contador
global de profundidade lança `RecursionError` a 1000 quadros, contando o
do módulo, como o CPython. O teste fica na chamada, depois dos
argumentos, e o erro aponta a linha da chamada. A função soma 1 ao
contador na entrada e tira 1 antes de cada `return` e no handler que
relança a exceção. O contador vai ser zerado a cada `wasm_call`. Uma
função folha, que não chama outra função Python, ainda conta, e tirar o
contador dela é uma otimização para depois.

O Stop usa a mesma comparação. O host zera o limite, e o caminho lento
confere a interrupção e dá trap por `unreachable`, que atravessa os
`except`. Os laços também testam o limite na cabeça. Na CLI, o
`max_wasm_stack` fica em 8 MiB. Uma função pequena usa de 64 a 96 bytes
por quadro, mas uma com 30 variáveis locais precisou de 256 a 512 KiB para
999 quadros, o que esgota os 512 KiB padrão do wasmtime. Se mesmo assim a
pilha estourar, o host traduz o trap ou o `RangeError` num
`RecursionError` sem traceback.

As chamadas entre funções Python não usam `return_call`, porque o
traceback precisa dos quadros.

### Memória esgotada

O wasm não captura a falta de memória. O wasmtime para com um trap ("GC
heap out of memory" ou "allocation size too large") e escreve o
backtrace dele, com o offset de cada quadro no módulo. O compilador
devolve, junto com o módulo, a faixa de código de cada site, e o host
troca o trap pelo traceback do CPython terminado em `MemoryError`. O
quadro interno dá o offset da instrução que falhou, e os outros o da
chamada. Nada disso custa em tempo de execução. A CLI pede 2000 quadros
ao wasmtime (`-D max-backtrace=2000`), porque o padrão é 20. Se o quadro
do módulo faltar, ou se algum offset cair fora de um site, o host mostra
o erro do wasmtime como veio, para nunca escrever um traceback errado.

O erro do wasmtime aparece em dois casos. O primeiro é a memória acabar
no handler que acrescenta o `$Frame` e relança, porque os quadros por
onde a exceção já passou ficam no heap e se perdem com o trap. O
segundo é ela acabar na construção dos literais, antes do programa.

O heap de GC do wasmtime tem teto de 4 GiB, e o `str` em UCS-4 usa 4
bytes por caractere. Um texto de cerca de mil milhões de caracteres dá
`MemoryError` aqui e funciona no CPython, que guarda 1, 2 ou 4 bytes por
caractere conforme o texto. Um `str` compacto, com as três larguras,
fecharia a diferença até um texto que o CPython também não comporta.

O V8 tem dois limites. Um array de 1 GiB ou mais, como `'a' * 2**28`,
lança um `RuntimeError` ("requested new array is too large") que o host
captura, e a pilha do erro traz o offset de cada quadro, então a mesma
faixa de código dos sites serve para o traceback. O CPython cria esse
`str`. Quando o heap inteiro acaba, perto de 4 GB, o V8 para o processo
com "Fatal JavaScript out of memory", e nem o host roda depois disso. No
navegador, a aba morre.

### Módulos e REPL

Até o REPL entrar, o programa e o runtime viram um módulo wasm só. Os
tipos e os intrínsecos do runtime são escritos em Rust, pelo mesmo
emissor, como no jwonder, e o que o Python consegue dizer fica no prelude,
em Python, a partir da fatia 1. Nenhum crate Rust é compilado para
`wasm32`.

As classes, as vtables, os singletons e os literais ficam em globais
`(mut (ref null T))`, preenchidos pela função de start e lidos com
`ref.as_non_null`. Um global com alocação no inicializador custa caro no
wasmtime. 100 globais `$Str` levaram 0,41 s para compilar, contra 0,09 s
pelo start, e 400 levaram 15 s, contra 0,21 s. Um literal `str` vem de um
segmento de dados por `array.new_data`, porque o V8 limita
`array.new_fixed` a 10 mil elementos.

Quando o REPL entrar, cada módulo Python vira um módulo wasm, que importa
o runtime e os módulos vizinhos. O runtime e o prelude são instanciados
uma vez por sessão e importados por handle, e o rec group fica congelado
por versão do prelude, porque os hosts recusam o link quando a cópia
difere. `Reprs` exporta uma impressão digital das representações, e o
REPL a confere.

Para isso, `Reprs` separa declarar uma função do runtime de defini-la,
porque o módulo do usuário só declara e importa. O `FuncId` de hoje só
nomeia funções de um `Program`, e o lowering precisa de uma sessão acima
de `lower()`, com os ids, os globais e os slots das entradas anteriores.

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

Na CLI, o wasmtime 48 roda hoje como processo externo, com
`-W gc=y,function-references=y,exceptions=y,max-wasm-stack=8388608`. Quando for embutido, ele
roda com as features `runtime`, `std`, `cranelift`,
`gc`, `gc-drc` e `parallel-compilation`, e o `cranelift_opt_level` fica em
Speed. Compilar um prelude do tamanho do da sondagem levou 0,2 s em 28
núcleos e 1,9 s num núcleo só. Quando o runtime virar um módulo separado,
ele vai ser compilado na primeira execução e guardado como `.cwasm` em
`config_dir()/spython/`, com a chave de
`Engine::precompile_compatibility_hash`.

### Regras de emissão

O Cranelift tem tempo superlinear em funções com muitas alocações. Por
isso as caixas são alocadas por chamada a um helper, e a inicialização de
um módulo é partida em funções de no máximo 64 alocações. A sondagem
mediu 400 alocações inline numa função em 5,4 s, e as mesmas 400 como
chamadas em 0,06 s.


## Testes

O oráculo é o CPython, para o stdout e também para as mensagens de erro e
o traceback no stderr. O oráculo dos doctests é o runner atual.

O corpus é o `na-programacao`, os programas dos branches antigos, o
`cli/tests/inputs`, os exemplos e as transcrições de REPL da documentação
e as sondagens. Cada programa roda num módulo próprio, no wasmtime e no
deno. Se o tempo dos testes pesar, as fixtures passam a ficar num módulo
só, atrás de um dispatcher.


## Fatias

A ordem segue o que o prelude precisa, e os níveis ordenam só o portão do
corpus.

- **−1. Sondagens.** Feitas, com o resultado na seção seguinte.
- **0a. CLI.** `def`, `int`, `if`, `while`, `print` e a exceção não
  capturada com linha, rodando no wasmtime externo. Já compila `int`,
  `bool` e `str` com seus operadores, `len`, `s[i]`, as variáveis do
  módulo e das funções, `global`, `if`, `while`, `break`, `continue`,
  `pass`, `def` com parâmetros posicionais, `return`, a recursão até o
  limite do CPython, o `NameError` com a sugestão do CPython, a memória
  esgotada e o traceback com as marcas do CPython. Defaults, argumentos nomeados,
  `def` dentro de função e função como valor ainda são recusados.
- **0b. Deno.** O mesmo no deno. Feita. Os testes do `compiler` rodam
  cada módulo no deno também, com o mesmo resultado do wasmtime.
- **1. Prelude.** Arrays, classes sem herança, a `$Class` e os métodos
  especiais. Depois o prelude de `str` e de `int`, e os doctests.
- **2. Níveis 0 e 1**, com o diferencial do corpus verde.
- **3. Nível 2.** Enum, dataclass e `match` com todos os padrões.
- **4. REPL na CLI.**
- **5. Nível 3.** `list`, `for`, `range`, fatias e `+=`.
- **6. Nível 4.** Classes com herança, `dict`,
  `set`, compreensões, lambda e closures. É a fatia de maior risco.
- **7. Imagens e programas interativos.**
- **8. Nível 5 por partes**, até o corpus passar. Depois os inteiros
  grandes e a saída do RustPython.
- **9. O wasmtime embutido na CLI.**
- **10. Navegador.** O compilador no crate `wasm/`, o `backend=wasmgc`
  no `repl_new`, o programa e o REPL na página. Fica por último, porque
  os programas standalone vêm primeiro.

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
- O tempo de compilar o runtime junto com cada programa. Se ele pesar na
  CLI, o runtime vira um módulo separado, com o `.cwasm` em cache, antes
  do REPL.
- O despacho direto quando a classe do receptor é conhecida, por causa do
  custo do `call_ref` no wasmtime.
- O que mostrar no Safari, que não dá offsets na pilha. O traceback
  montado no desenrolar não depende deles, mas o trap de Stop e o estouro
  de pilha dependem do host.
- As imagens: chamar o Image em Rust do sinteract pela fronteira ou
  reescrever a biblioteca no prelude.
- O `str` compacto, com 1, 2 ou 4 bytes por caractere, para o teto de
  4 GiB do heap do wasmtime.
- O heap do V8 que acaba e derruba o processo, e o limite de 1 GiB por
  array do V8, abaixo do que o CPython aceita.
- A memória que acaba no handler que relança uma exceção ou na
  construção dos literais, que hoje mostra o erro do wasmtime.
- Como um método dos stubs do typeshed, como `str.upper`, chega à
  definição dele no prelude sem passar pelo nome impresso.
- Um módulo `host/` com um arquivo por host, quando houver um segundo.
  Hoje o `trap.rs` lê só o stderr do wasmtime externo.
