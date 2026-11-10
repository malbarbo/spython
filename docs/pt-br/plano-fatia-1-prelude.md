# Plano: a fatia 1, o prelude

Este plano detalha a fatia 1 do [plano do backend
WasmGC](plano-backend-wasmgc.md): "Arrays, classes sem herança, a `$Class`
e os métodos especiais. Depois o prelude de `str` e de `int`, e os
doctests."

Em 9/nov/2026 o usuário decidiu tirar desta fatia as classes, os métodos
especiais e os arrays, e levar cada um à fatia que o usa. A seção "O que
sai da fatia" diz para onde cada item vai.

## Onde a fatia começa

O compilador aceita `int`, `float`, `bool` e `str` com os operadores,
`def` com parâmetros posicionais desses quatro tipos, `if`, `while`,
`print`, `math`, `str(x)` e f-strings sem formato. A `Rep` tem cinco
formas (`Int`, `Float`, `Bool`, `Str`, `None`). A `$Exc` guarda só a
última linha do traceback, já pronta.

Os níveis 0 e 1 são os capítulos 01 a 04 do `na-programacao`, com 28
arquivos. Desses, 27 têm doctests e só 2 chamam `print`. Os 134 exemplos
esperam 42 `int`, 30 `str`, 25 `bool` e 19 `float`, sem `Traceback` e sem
`<BLANKLINE>`. Fora dos doctests, esses arquivos pedem a fatia de `str`
(9 arquivos, sem passo), `upper` e `lower` (1 arquivo) e `assert` (1
arquivo). Nenhum usa `input`, f-string, argumento nomeado ou união.

O teste do corpus de hoje não roda doctests. Um arquivo só com funções e
doctests conta como igual ao CPython com a saída vazia.

## O que a fatia entrega

1. As chamadas completas: argumentos nomeados, parâmetros só nomeados e
   defaults.
2. `raise` de uma exceção builtin e `assert`, com a `$Class` das exceções.
3. O mecanismo do prelude: funções em Python que o compilador compila
   junto com o programa, só as que o programa alcança, sem quadro no
   traceback.
4. Os doctests pelo backend, com o relatório do runner atual, e o portão
   de doctests do corpus.
5. A fatia de `str`, `upper` e `lower`, que fecham os níveis 0 e 1.
6. A forma uniforme, com `T | None` e `int | float`.
7. O resto de `str`, e `int(str)` e `float(str)`.

## O que sai da fatia

O plano do backend põe na fatia 1 as classes, os métodos especiais e os
arrays, porque o prelude ia precisar deles. O prelude desta fatia é feito
de funções, e esses três itens não têm quem os use antes das fatias
abaixo. O corpus inteiro não define nenhum método especial fora do
`__init__`.

| Item | Vai para | Quem usa primeiro |
|---|---|---|
| classe com atributos e `__init__` | 3 | dataclass e Enum (nível 2) |
| `Array` e `$Values` | 5 | a `list` do prelude |
| métodos especiais e métodos de classe | 5 | a `list` do prelude, e o aluno no nível 4 |
| `input` | 5 | capítulo 09 |
| formato de f-string e `format` | 5 | `{n:03d}`, o único formato do corpus |
| `bin`, `oct`, `hex`, `bit_length`, `bit_count` | 8 | nenhum uso no corpus |
| `hash` | 6 por dentro, 8 como builtin | `dict` e `set` |

O `hash` do CPython vai até 2^61 e o de `str` usa 64 bits. `hash("a")` é
4644417185603328019, e não cabe no `int` de 31 bits. Por dentro, `dict` e
`set` usam um `i64`. O builtin espera os inteiros grandes.

## Desenho

### Chamadas

Hoje o lowering guarda as funções pelo nome e conhece um arquivo só. Três
refatorações vêm antes de tudo: as funções passam a ser identificadas
pelo `DefId`, o contexto de um arquivo (`model`, `source`, `path`) se
separa do estado do programa, e o laço que compila as funções vira uma
fila.

O `call_bindings` do `ty` dá o parâmetro de cada argumento. Ele não diz
nada sobre os parâmetros que faltam, então o default vem da assinatura que
o compilador já guarda. Um argumento nomeado vai para a posição do
parâmetro, e os argumentos são avaliados na ordem em que aparecem. O teste
de `NameError` do nome da função vem antes dos argumentos. O default é
avaliado uma vez, quando o `def` roda, e fica num global. O compilador já
recusa dois `def` com o mesmo nome, e por isso um global por parâmetro
basta. `*args` e `**kwargs` continuam fora.

### `raise` e `assert`

A `$Exc` passa de `{mensagem, tb}` para `{class, arg, tb}`, com `arg` um
`str` anulável. Ela cresce para o layout do plano do backend (`vt`,
`args`, `ctx`) na fatia 6, com o `except`. A `$Class` entra agora, com o
nome, a profundidade e os ancestrais, e cada exceção builtin é uma. A
última linha do traceback sai do nome da classe e de `arg`.

Antes disso, um commit separa cada mensagem de hoje em classe e texto, em
`cpython/`, no `NameError`, no `trap.rs` e no `OSError`.

`raise C(msg)` e `raise C` valem para as exceções builtin, com `msg` um
`str`. `assert c` e `assert c, msg` lançam `AssertionError`. As marcas do
traceback de um `assert` ficam sob a condição, não sob o comando.

### O mecanismo do prelude

O prelude é um pacote, `__spython_prelude__`, com um arquivo por assunto
(`str.py`, `int.py`, `float.py`, `math.py`, `doctest.py`). O lint
`reserved-name` do engine já rejeita o `import` de um nome com o prefixo
`__spython_`, e nada muda nos lints.

**Onde os arquivos ficam.** Num crate pequeno, `compiler/prelude/`, sem
dependências, que exporta a lista dos arquivos com o nome e o texto. O
`engine` depende dele, e os três construtores de banco
(`in_memory_project_at`, o overlay nativo e o `ReplProject`) escrevem os
arquivos. O `compiler` os encontra pelo banco, como qualquer módulo.

**Checagem.** O `ty` infere os tipos do prelude como os de qualquer
arquivo. Os lints de ensino não rodam sobre ele. Um teste checa o pacote
sem nenhum diagnóstico.

**O dialeto.** O prelude só usa o que o compilador já aceita. Nesta fatia
isso é `while`, `if`, `int`, `float`, `bool`, `str` e as funções do
próprio prelude. Não há `for`, `list`, tupla nem `+=`.

**Intrínsecos.** O que precisa de instrução wasm fica em
`__spython_prelude__/intrinsics.pyi`, e cada função dele é um nó da IR. O
lowering a reconhece pelo `DefId`. Nesta fatia os intrínsecos são o
`StrBuilder` (`builder_new(n)`, `b[i] = cp`, `builder_done(b)`), a
leitura de um code point (`code_point(s, i) -> int`, sem alocar o `str`
de um caractere que `s[i]` aloca) e as tabelas do Unicode.

**Do stub ao prelude.** `s.upper()` chega como `CallTarget::Method` com
`class.known == Some(KnownClass::Str)`. `ord(c)` chega como
`Function(DefId)` no `builtins.pyi`, e `int("3")` como
`Constructor(ClassId)`. Uma tabela em `lower/prelude.rs` leva cada um à
função do prelude. Um receptor literal, como `"abc".startswith("a")`,
pode chegar como `KnownBoundMethod` e cair em `CallTarget::Other`, e um
teste no fork confere isso antes da etapa G. A função do prelude tem a
assinatura que o prelude consegue escrever. A tabela diz como os
argumentos do stub (com `SupportsIndex`, sobrecargas e defaults `None`)
viram os dela, e não há teste automático de paridade com o stub.

**Só o que o programa alcança.** A fila compila uma função do prelude
quando a primeira chamada a ela aparece. Um teste confere o tamanho do
módulo de `print("oi")` e o de `print(repr("a"))`.

**Sem quadro no traceback.** No CPython um builtin não aparece no
traceback. Uma função do prelude é emitida sem `enter`, sem `try_table`,
sem a conta de recursão e sem `SiteFrame`, e a chamada a ela não testa o
limite de recursão. Um `raise` dentro dela não precisa de site, porque o
handler de quem chamou acrescenta o quadro com o próprio `$site`. Na
memória esgotada, o `trap.rs` já pula uma função sem quadro registrado.
Isso está certo para uma chamada explícita, como `"abc".index("z")`. Uma
entrada implícita (a verdade de um valor, `==`, um campo de f-string)
precisa de um site com a faixa do operando, e só aparece com os métodos
especiais, na fatia 5.

**Fronteira com o código à mão.** A primeira função que sai do emissor
para o prelude é `math.isnan`, e o commit mede o tamanho do módulo e o
tempo de 10^7 chamadas nas duas formas. Se o Python empatar, `isinf`,
`isfinite`, `degrees`, `radians`, `isqrt` e `factorial` seguem, um commit
cada. Se perder, o plano registra o motivo e as funções ficam à mão. Com
o `int` de 31 bits, `isqrt`, `factorial` e `int(str)` são laços curtos, e
a regra do crate testado volta a valer para eles com os inteiros grandes.

### Os doctests

O runner atual (`engine/src/doctest_runner.py`) usa `list`, `split`,
`exec` e `%`. Ele não compila nesta fatia, e o `exec` nunca vai compilar.
O trabalho se divide assim:

- **No `engine`, em Rust, na compilação.** A separação dos exemplos, o
  texto esperado de cada um, a troca do `<BLANKLINE>` e a linha da exceção
  de um exemplo que espera `Traceback`. Hoje o `DoctestSnippet` junta os
  exemplos de uma docstring num corpo só, e ganha as fronteiras. O teste
  compara o resultado com o `_parse` do runner em todo o corpus.
- **No compilador.** Cada exemplo vira um comando novo da IR, que roda o
  corpo, captura a `$Exc` e a entrega ao relatório. Dentro do exemplo,
  todo comando de expressão escreve o `repr` do valor que não é `None`,
  inclusive dentro de um `if` ou de um `while`, como o modo `single` do
  CPython. Toda variável da função sintética tem a flag de valor, porque
  um exemplo que lança deixa as seguintes sem valor, e a leitura lança
  `NameError`, como no runner.
- **No módulo.** A saída capturada fica num buffer do próprio módulo, que
  cresce. O host não devolve texto ao módulo.
- **No prelude, em Python.** A comparação de dois `str` e o texto de uma
  falha, com `while` e o `StrBuilder`.

O relatório copia o runner: a ordem das docstrings (o módulo, depois por
nome), a linha `Running tests...`, o resumo, o código de saída, a mensagem
de um arquivo que lança antes dos testes, e `__name__ == "__doctest__"`.
A CLI ganha o `--backend=wasmgc` no comando `test`, oculto como no `run`.

Um trap não é uma `$Exc`. Se um exemplo sai dos 31 bits, esgota a memória
ou estoura a pilha, o programa para ali, como qualquer programa, e os
exemplos seguintes não rodam. O runner atual tem inteiros grandes e passa
nesses exemplos. O portão deixa de fora o arquivo em que isso acontece,
até os inteiros grandes.

O portão é um teste que roda os arquivos com doctest do corpus nos dois
backends e compara a saída e o código de saída. O número de arquivos
iguais só pode subir.

### `str`

A fatia `s[a:b]` fica à mão, em `StrRepr`, porque é um `array.copy`. A
fatia com passo fica no prelude.

As tabelas têm de ser as do Unicode 15.1, a versão do CPython 3.13. Elas
ficam no crate dos builtins, geradas no `build.rs` a partir dos arquivos
do UCD, que ficam em `compiler/builtins/ucd/` com a licença Unicode-3.0.
Um teste as compara com o CPython nos 1.114.112 code points. Os arquivos
são `UnicodeData.txt`, `SpecialCasing.txt`, `CaseFolding.txt`,
`DerivedCoreProperties.txt` e `DerivedNumericValues.txt`, cerca de 3,5 MB
de texto. Cada propriedade custa de 3 a 6 KB no módulo que a usa. O
`repr` de `str` puxa a tabela de `isprintable`, e por isso ela entra
cedo.

Os casos que os testes têm de cobrir: `'ß'.upper()` é `'SS'`, o sigma
final de `lower` depende de `Cased` e `Case_Ignorable`, `title` e
`capitalize` usam a caixa de título, `'a'.center(4, '*')` é `'*a**'`, e o
conjunto de espaços de `strip` difere do de `int()` e `float()`.

### A forma uniforme

O `ty` do fork devolve `Shape::Float` para toda união `int | float`, e o
compilador não vê a diferença. Um commit no fork faz `shape()` devolver a
união quando `strict_numbers` está ligado.

A `Rep` continua `Copy`. Ela ganha uma forma de referência anulável e
`Uniform`, que é `(ref null eq)`. A conversão é um nó explícito da IR,
`Box` e `Unbox`, com a tabela do plano do backend (`int` em `i31ref`,
`float` em `$Float`, `bool` num dos dois `$Bool`, `None` em `null`).

O `ty` estreita o tipo de uma variável local, e o compilador confia nele.
Ele também estreita um atributo e uma variável global, e uma chamada no
meio não desfaz o estreitamento. Por isso a leitura de uma global que
alguma função atribui usa a `Rep` declarada e um `Unbox` que confere o
tipo e lança o erro do CPython. A mesma regra vale para os atributos na
fatia 3.

Com `int | float` uniforme, `int ** int` com expoente variável deixa de
parar o programa (decisão 11 do plano do backend).

### `int(str)` e `float(str)`

`int(' ٣_4 ')` é 34. A gramática aceita espaços, sinal, `_` entre dígitos
e os dígitos decimais do Unicode, e a mensagem de erro traz o `repr` do
argumento. Ele fica no prelude, em Python. `float(str)` usa o `fast-float2` do crate, com a gramática do
CPython na frente.

## Etapas

Cada linha é um commit, e cada commit passa pelo portão rápido. Onde uma
linha tem mais de um método, eles dividem um algoritmo.

**A. Chamadas**

1. `compiler: name a function by its DefId in the lowering`
2. `compiler: split the context of a file from the lowering`
3. `compiler: lower the functions from a queue`
4. `compiler: bind the arguments of a call by call_bindings`
5. `compiler: compile a call with keyword arguments`
6. `compiler: compile a keyword-only parameter`
7. `compiler: compile a parameter with a default`

**B. Exceções**

8. `compiler: split the message of each exception from its class`
9. `compiler: add $Class with the builtin exceptions`
10. `compiler: keep the class and the argument in $Exc`
11. `compiler: compile raise of a builtin exception`
12. `compiler: compile assert`

**C. O mecanismo do prelude**

13. `engine: add the prelude package to the database of the checker`
14. `compiler: lower a function of the prelude when a call reaches it`
15. `compiler: emit a function of the prelude without a frame`
16. `compiler: lower a function of the intrinsics as a node of the IR`
17. `compiler: move math.isnan to the prelude`, com a medida, e depois um
    commit para cada uma das outras seis funções de `math`

**D. Doctests**

18. `compiler: compile repr of an int`
19. `compiler: compile repr of a float`
20. `compiler: compile repr of a bool`
21. `compiler: add StrBuilder and code_point to the intrinsics`
22. `compiler: add the table of printable code points to the builtins`
23. `compiler: compile repr of a str`
24. `engine: split the examples of a docstring in Rust`
25. `compiler: capture the output in a buffer of the module`
26. `compiler: add the statement that runs an example of a docstring`
27. `compiler: compile the synthetic function of a docstring`
28. `compiler: add the report of a failed example to the prelude`
29. `compiler: run the docstrings of a file as the runner does`
30. `cli: add --backend=wasmgc to the test command`
31. `compiler: compare the doctests of the corpus with the runner`

**E. O `str` dos níveis 0 e 1**

32. `compiler: compile a slice of a str`
33. `compiler: add the case tables of Unicode 15.1 to the builtins`
34. `compiler: add str.upper and str.lower to the prelude`

**F. A forma uniforme**

35. `ty: keep int | float as a union under strict_numbers`, no fork
36. `compiler: add the rep of a str or None`
37. `compiler: add Rep::Uniform with Box and Unbox`
38. `compiler: compile is None and is not None`
39. `compiler: compile a parameter and a result of a union`
40. `compiler: print a value of a union`
41. `compiler: compare a value of a union`
42. `compiler: compile int ** int with a variable exponent`

**G. O resto de `str`**

43. `find`, `rfind`, `index`, `rindex`
44. `count`
45. `startswith`, `endswith`, com um `str`
46. `in` entre dois `str`
47. `replace`
48. `removeprefix`, `removesuffix`
49. `strip`, `lstrip`, `rstrip`
50. `center`, `ljust`, `rjust`
51. `zfill`
52. `expandtabs`
53. a fatia com passo
54. `capitalize`, `title`, `swapcase`
55. `casefold`
56. `isalpha`, `isdecimal`, `isdigit`, `isnumeric`, `isalnum`, `isspace`
57. `isupper`, `islower`, `istitle`
58. `isascii`, `isprintable`, `isidentifier`
59. `ord`
60. `chr`

**H. `int` e `float` a partir de `str`**

61. `compiler: compile int of a str`
62. `compiler: compile int of a str with a base`
63. `compiler: compile float of a str`

**I. Fecho**

64. `docs: record the slice 1 in the plan of the backend`

As etapas A a E já deixam a fatia 2 quase pronta. Falta nela o mapa dos
capítulos para os níveis no teste do corpus.

## Testes

- Cada commit traz os testes do que ele compila, em `compiler/tests/run/`,
  com o CPython como oráculo do stdout, do stderr e do código de saída,
  cada fluxo comparado em separado, no wasmtime e no deno.
- Cada função do prelude tem os casos de borda do `Lib/test`
  correspondente que cabem no subconjunto.
- As funções de um code point são comparadas com o CPython em todos os
  code points.
- O teste de tamanho do módulo e o portão dos doctests, descritos acima.

## Riscos

- **O `ty` do engine roda como Python 3.14** (`engine/src/check.rs:111`),
  e o oráculo é o 3.13.5. Uma diferença nos stubs pode aparecer na tabela
  do stub ao prelude.
- **O tamanho e o tempo do prelude.** A medida do commit 17 decide. Se o
  Python perder por causa do teste de 31 bits, o prelude precisa de um
  modo sem esse teste, e isso é uma decisão nova.
- **As tabelas do Unicode no módulo.** Falta conferir que o ligador
  descarta os dados de uma função do crate que o programa não chama.
- **O relatório dos doctests.** O runner escreve no stderr e no stdout, e
  a ordem entre os dois tem de ser a mesma com a captura dentro do módulo.

## O que a revisão mudou

- Os doctests subiram do fim para a etapa D, porque são o portão real da
  fatia 2 e dependem só das etapas A a C e do `repr`.
- As classes, os métodos especiais e os arrays saíram, sem consumidor. As
  comparações de classe do CPython não são uma chamada direta
  (`NotImplemented`, o operando trocado, `__ne__` derivado), e o
  `AttributeError` tem sugestão calculada em tempo de execução. Os dois
  ficam para o plano das fatias 3 e 5.
- O `hash`, o `input`, o formato e `bin`/`oct`/`hex` saíram.
- A `$Exc` nova dependia de tipos que vinham depois. A etapa B agora os
  traz.
- O lowering não suporta a fila nem dois arquivos, e três commits de
  refatoração abrem a etapa A.
- O `doctest.py` deixou de ser um porte do runner.
- O `Unbox` sem teste estava errado para globais e atributos.
- `int | float` precisa de um commit no fork.
- O commit que pulava os quadros do prelude na memória esgotada sumiu,
  porque o `trap.rs` já faz isso.

## O que a implementação mudou

A fatia foi feita em 10/nov/2026, em 70 commits no spython e um no fork.
Alguns commits do plano se juntaram, porque o código de um deles ficava
morto sem o outro e o clippy recusa código morto. São o 9 com o 10, o 16
com o 21, os 18 a 20, os 25 a 29, e o 61 com o 62. O 39 veio dentro do 36
e do 37, porque um parâmetro e um resultado de uma união já passam pela
mesma conversão das atribuições. Um commit novo tira o site de uma
chamada do prelude que não pode lançar, e sem ele cada `math.isnan`
custava cerca de 500 bytes.

O teste do `engine` compara a separação dos exemplos com o `_parse` do
runner em 3000 docstrings aleatórias, e não só no corpus. O oráculo dos
testes de doctest do compilador é o runner rodando no CPython, com um
substituto do `_spython_ffi`. O `SyntaxError` e as classes derivadas
ficaram fora do `raise`, porque o CPython escreve `<no detail available>`
para elas sem texto.

`upper` e `lower` são duas funções, para que um programa leve só a tabela
da que chama, e os seis predicados de classe de `str` têm um laço cada um
pelo mesmo motivo. Uma tabela nova do Unicode entrou no crate dos
builtins antes de cada método que a usa, sempre com o teste contra o
CPython em todos os code points. O conjunto dos espaços e a categoria
`Lt` são pequenos e ficaram em Python, com a mesma conferência.

O `Unbox` entrou com o `is None`, que é o primeiro teste que estreita uma
união. O compilador confia no estreitamento de uma variável local e no de
uma global que nenhuma função declara com `global`. Uma chamada pode
mudar outra global entre o teste e a leitura, e essa leitura continua
recusada, em vez do `Unbox` com conferência que o plano previa. `print`
aceita `None` também.

O compilador recusava `int ** int` inteiro, e não só com o expoente
variável. A potência com um expoente que não é negativo ficou no emissor,
num `i64`, porque a mensagem do limite tem de ser a de `lcm` e de `comb`
(`the result of ** does not fit in an int`), e não a de um quadrado no
meio da conta. O `power` do prelude chama essa potência e dá um `float`
para um expoente negativo.

Um `str` do spython não guarda um surrogate, e o CPython só o recusa na
escrita. `chr` de um surrogate para o programa com um erro do spython,
pelo intrínseco novo `stop`. `float(str)` confere a gramática no prelude e
copia o texto para a memória linear depois da área do host, que cresce
para um texto longo, onde o `fast-float2` o lê.

A conversão `!r` de um campo de f-string entrou junto. Uma chamada de
método com argumento nomeado continua recusada, e isso inclui o `count=`
de `replace`, que o CPython 3.13 aceita. Uma função do prelude não tem
defaults, e o prelude passa o `None` de cada parâmetro opcional.

## Decisões

O usuário tomou estas decisões em 9/nov/2026, uma pergunta por vez.

1. As classes, os métodos especiais e os arrays saem da fatia 1 e vão
   para a fatia que os usa.
2. Os arquivos do prelude ficam num crate só com os textos.
3. `isqrt`, `factorial` e `int(str)` ficam em Python, no prelude.
   `float(str)` continua no `fast-float2`.
4. Um exemplo de doctest que sai dos 31 bits para o programa, como em
   qualquer programa. O portão deixa o arquivo de fora até a fatia 8.
5. Os cinco arquivos do UCD 15.1 ficam no repositório.
6. O `lowering.rs` do fork muda para devolver a união `int | float` com
   `strict_numbers` ligado.
