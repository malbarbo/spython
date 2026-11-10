# Plano: a fatia 3, os tipos do usuário

Este plano detalha a fatia 3 do [plano do backend
WasmGC](plano-backend-wasmgc.md): "Nível 2. Classes sem herança, Enum,
dataclass e `match` com todos os padrões."

Em 10/nov/2026 o usuário decidiu levar o `match` para depois. A seção
"O `match`, para depois" guarda o desenho que a revisão conferiu.

O compilador reconhece uma forma fixa de escrever uma dataclass e um
Enum. O programa compilado não roda o decorador, a superclasse nem as
anotações. O compilador lê a forma no texto, pede os tipos ao `ty` e gera
o construtor, o `repr` e o `==`, como o `dataclasses` e o `enum` do
CPython fariam. Uma classe fora dessa forma é recusada com
`Unsupported`.

## Onde a fatia começa

O compilador cobre os níveis 0 e 1. O teste do corpus verifica cada
capítulo no nível dele e falha quando o compilador recusa um arquivo de
nível 0 ou 1. A `Rep` tem sete formas (`Int`, `Float`, `Bool`, `Str`,
`None`, `StrOrNone` e `Uniform`). A forma uniforme é um `(ref null eq)`
sem vtable, e `print`, `repr` e `==` de um valor uniforme escolhem o tipo
por `ref.test`. Cada tipo wasm sai num rec group próprio, e o módulo não
tem `sub`, `ref.func` nem `call_ref`. O compilador só aceita o import de
`math`.

O nível 2 é o capítulo 05 do `na-programacao`, com 10 arquivos. O checker
recusa 2 deles no nível 2. `janela.py` usa uma comparação encadeada, e
`resolucao.py` tem membros de Enum fora de maiúsculas. Os 8 restantes
usam:

- `from enum import Enum, auto` e `from dataclasses import dataclass`;
- 6 Enums, todos com `auto()`, e `.name` nos exemplos dos doctests;
- 4 dataclasses, todas com `@dataclass` sem argumentos, construídas por
  posição e por nome;
- `==` entre membros de Enum, em cadeias de `if`/`elif` que o `ty` prova
  completas, com o resultado numa variável ligada em cada ramo e lida
  depois da cadeia;
- o `repr` de uma dataclass nos exemplos, como
  `Tempo(horas=0, minutos=2, segundos=40)`.

Nenhum arquivo do `na-programacao` usa `match`, nem os `.py` nem as
notas de aula. `dataclasses.replace` aparece só em `teste-snake.py`, na
raiz do livro, que precisa de `sys` e de listas. `IntEnum`, `StrEnum` e
uma classe com base própria só aparecem em `cli/tests/inputs`, que o
corpus verifica no nível 5.

## O que a fatia entrega

1. O import de `enum` e de `dataclasses`.
2. A forma aceita de dataclass e de Enum, com uma mensagem de
   `Unsupported` para o que fica de fora.
3. As classes do usuário como structs num rec group, sem vtable.
4. O Enum, com os membros, `.name`, `.value`, o `repr`, o `str`, o `==`
   e o `is`.
5. A dataclass, com o construtor por posição, por nome e com defaults, a
   leitura e a escrita dos campos, o `repr` e o `==`.
6. O capítulo 05 no portão do corpus, no nível 2.

## O que sai da fatia

| Item | Vai para | Por quê |
|---|---|---|
| classe sem `@dataclass` e sem `Enum` | 6 | sem método, ela só guarda atributos de classe, e nenhum arquivo do corpus a usa |
| herança, inclusive de uma dataclass | 6 | nível 4 |
| `$VT`, `$Object` e `call_ref` | 6 | sem herança, o `ref.test` decide a classe |
| métodos numa dataclass ou num Enum | 6 | o checker só aceita método no nível 4 |
| `match` | depois desta, numa fatia própria | nenhum arquivo do corpus o usa (decisão 1) |
| `for c in Cor`, `Cor(1)`, `Cor['X']`, `len(Cor)` | 5 | precisam de `for` e de iteração, sem uso no corpus |
| `hash` de um objeto | 6 | `dict` e `set` |
| f-string com formato de um objeto | 5 | o formato de f-string inteiro vai para a 5 |
| `dataclasses.replace` | 8 | o `ty` não confere os argumentos, e o único uso precisa de `sys` |
| `IntEnum`, `StrEnum`, `Flag` | 8 | só em `cli/tests/inputs` |
| `field()`, `ClassVar`, `__post_init__`, `order=True` | 8 | sem uso no corpus |
| `isinstance` e `type` | 8 | sem uso no corpus |
| copiar um nome de classe num exemplo de doctest | 8 | a cópia de um nome de função já é `Unsupported` |

## Desenho

### Os imports

`import enum`, `import dataclasses`, `from enum import Enum, auto` e
`from dataclasses import dataclass` entram como o import de `math` entra
hoje, com `as` e com a flag do nome. Nenhum desses imports roda código
no programa compilado. `from enum import *` e qualquer outro nome dos
dois módulos são `Unsupported`.

### A forma aceita

Uma dataclass é escrita assim:

```python
from dataclasses import dataclass

@dataclass
class Tempo:
    """Docstring opcional."""

    horas: int
    minutos: int = 0
```

- O decorador é um só, `dataclass`, `dataclass()` ou
  `dataclasses.dataclass`. O `ty` confirma que o nome é a função de
  `dataclasses`, por `expr_shape` com um `is_dataclasses` como o
  `is_builtins` de hoje, e que `class_info().dataclass_fields` existe.
  O decorador aceita só `frozen=True` como argumento (decisão 2).
- A classe não tem base, keyword na lista de bases nem outro decorador.
- O corpo tem só uma docstring, `pass`, `...` e campos anotados, com ou
  sem default. O tipo do campo vem do `ty`, e a anotação não é avaliada.
- O default é avaliado no `class`, na ordem do texto, como o corpo da
  classe faria. Um default que lê o nome de um campo anterior é
  `Unsupported`, porque no CPython ele lê o default desse campo, e não o
  global de mesmo nome.
- Um default cujo tipo é uma dataclass lança, no `class`, o
  `ValueError: mutable default <class 'T'> for field t is not allowed:
  use default_factory` do CPython, com o traceback dele.
- Uma anotação sem aspas é avaliada pelo CPython 3.13 no `class`. Um nome
  nela que ainda não tem valor, como a própria classe em
  `prox: No | None`, lança `NameError` ali. O compilador não avalia a
  anotação, mas testa a flag de cada nome dela e lança o mesmo
  `NameError`, com o traceback do CPython. Uma anotação em aspas não é
  testada.

Um Enum é escrito assim:

```python
from enum import Enum, auto

class Cor(Enum):
    """Docstring opcional."""

    VERDE = auto()
    AMARELO = 2
```

- A base é uma só, `Enum` ou `enum.Enum`, e o `ty` confirma que ela é
  `enum.Enum`, pelo `ClassId.known` de `class_info().mro`.
- O corpo tem só uma docstring, `pass`, `...` e atribuições
  `NOME = valor`. O nome não começa com `_`.
- Os valores são todos `int` ou todos `str`. Um valor `int` é `auto()`
  ou um literal, com `-` ou sem. Um valor `str` é um literal. `X: int =
  1`, um `float`, um `bool` e uma mistura de `int` com `str` são
  `Unsupported`.
- O compilador calcula `auto()` pela regra do CPython 3.13. O primeiro
  vale 1, e cada outro vale o maior valor anterior mais 1. O `ty` erra
  essa regra quando os valores não sobem, e com `X = 5`, `Y = 1`,
  `Z = auto()` ele dá a `Z` o valor 1 e o trata como apelido de `Y`. O
  CPython dá 6. O compilador compara o valor que calcula com o literal
  que o `ty` dá a cada membro, e uma diferença é `Unsupported`, porque o
  `ty` raciocina sobre um Enum que não é o do programa.
- Dois membros com o mesmo valor são apelidos no CPython, e são
  `Unsupported`.

O texto é a fonte da forma, e o `ty` é a fonte dos tipos e da resolução
dos nomes. Um nome sombreado, como uma função local chamada `dataclass`,
falha na confirmação pelo `ty` e é recusado.

O `class` em si avalia os defaults, testa os nomes das anotações e marca
a flag do nome da classe. Usar a classe antes do `class` lança
`NameError`, pela flag, como já acontece com uma função antes do `def`.

### Representação

| Tipo do `ty` | `Rep` | Forma local | Forma uniforme |
|---|---|---|---|
| instância de dataclass `C` | `Object(C)` | `(ref null $C)`, lido com `ref.as_non_null` | `(ref $C)` |
| membro de Enum `E` | `Object(E)` | igual | `(ref $E)` |
| `C \| None` | `ObjectOrNone(C)` | `(ref null $C)` | igual |

`C` é um índice na tabela de classes da IR, e a `Rep` continua `Copy`.
`rep_of` dá `Object(C)` a `Shape::Instance` de uma classe do programa, a
`Shape::EnumMember` e a uma união de membros do mesmo Enum, que é o tipo
de uma variável depois de `if c == Cor.A`. `join` dá `ObjectOrNone(C)` a
`Object(C)` com `None`, como dá `StrOrNone` hoje.

As classes do programa ficam num rec group só, porque uma dataclass pode
ter um campo de outra ou dela mesma. Dentro de um rec group, dois structs
de mesma forma são tipos distintos, pela posição no grupo. Por isso um
`ref.test $Tempo` decide se um valor uniforme é um `Tempo`, e esta fatia
não precisa de vtable.

```wasm
(rec
  ;; um por Enum, com o valor na forma local
  (type $Cor (struct (field $name (ref $Str)) (field $value i32)))
  ;; um por dataclass, com cada campo na forma local da Rep dele
  (type $Tempo (struct (field (mut i32)) (field (mut i32)) (field (mut i32)))))
```

- `Reprs::new` recebe a tabela de classes e emite o grupo antes dos
  globais das variáveis, porque a forma local de `Object(C)` já precisa
  do tipo de `$C`.
- Os membros de um Enum ficam em globais mutáveis e nulos, preenchidos
  no `_start` antes do código do módulo, como as classes das exceções
  hoje. O nome de um membro é um `$Str`, e o `array.new_data` dele não é
  uma expressão constante. Cada leitura faz `ref.as_non_null`.
- Os campos de uma dataclass são `mut`. Uma `frozen=True` não muda o
  struct, porque o checker recusa a atribuição a um campo dela.

### Despacho

`print`, `str`, `repr` e `==` de um valor uniforme testam primeiro os
tipos builtin, como hoje, e depois `ref.test` contra cada classe do
programa, numa cadeia. Um valor de `Rep::Object` chama a função da
classe direto. A verdade de um objeto é sempre `True`, porque nenhuma
das duas formas define `__bool__` ou `__len__`, e um membro com valor 0
também é verdadeiro.

### O Enum

- `Cor.VERDE` lê o global do membro.
- `.name` lê `$name`, e `.value` lê `$value`.
- `repr` escreve `<Cor.VERDE: 1>`, com o `repr` do valor. `str`,
  `print` e f-string sem formato escrevem `Cor.VERDE`.
- `==`, `!=`, `is` e `is not` entre membros são `ref.eq`. Um membro
  comparado por `==` com um valor de outro tipo, como `Cor.VERDE == 1`,
  dá `False`.

### A dataclass

- **Construtor.** `C(...)` chega como `CallTarget::Constructor`, que
  `call()` hoje não trata. Um ramo novo liga cada argumento ao campo pelo
  `parameter_name` de `call_bindings`, e não pelo índice, que pode contar
  o `self`. Um campo sem argumento lê o global do default. A construção
  é um `struct.new` em linha. O checker já recusa argumento faltando, a
  mais, repetido ou desconhecido, então o construtor não lança.
- **Campos.** `c.campo` é `struct.get`, e `c.campo = v` é `struct.set`.
  O `ty` resolve o nome pelo `attribute_owner`. Num `ObjectOrNone`, o
  checker só aceita o acesso depois de estreitar o valor, e o
  compilador faz `ref.as_non_null`.
- **`repr`.** Uma função gerada por classe escreve
  `Nome(campo=repr, ...)`. O CPython protege o `repr` com
  `reprlib.recursive_repr`, que escreve `...` quando o mesmo objeto volta
  durante o próprio `repr`. A função gerada guarda os objetos em curso
  numa pilha global, que precisa de um array que cresce. No traceback,
  cada nível do `repr` gera dois quadros sem colapso,
  `File "/usr/lib/python3.13/reprlib.py", line 21, in wrapper` e
  `File "<string>", line N, in __repr__`, com N = 5 + o número de campos.
  Um `repr` fundo estoura a pilha com metade da profundidade de uma
  função comum.
- **`==`.** Uma função gerada por classe faz o que o `__eq__` do CPython
  3.13 faz. Ela devolve `True` quando os dois são o mesmo objeto, depois
  `False` quando a outra classe é diferente, e depois compara os campos
  em ordem. Com isso, `n == n` é `True` mesmo com um campo `nan`, e um
  objeto com ciclo é igual a si mesmo sem recursão. Só dois ciclos
  distintos lançam `RecursionError`. O quadro é
  `File "<string>", line N, in __eq__`, com N = 10 + o número de campos,
  e o traceback resume as linhas repetidas como já faz.

### O `match`, para depois

Esta seção não entra na fatia. Ela guarda o desenho que a revisão
conferiu contra o CPython 3.13. Os padrões de sequência e `*` precisam
de `list` e de tupla, e o de mapeamento precisa de `dict`.

O sujeito é avaliado uma vez num local. Cada `case` vira um teste e um
bloco, na ordem do texto, e o primeiro que casa e passa na guarda roda.
Sem caso que case, o `match` não faz nada.

| Padrão | Teste |
|---|---|
| literal de `int`, `float` ou `str`, com `-` ou sem | `==` com o literal |
| `None`, `True`, `False` | `is` |
| valor com ponto, como `Cor.VERDE` | `==` com o valor |
| captura e `_` | casa sempre, e a captura liga o nome |
| `p1 \| p2` | o primeiro que casa |
| `p as nome` | casa `p` e liga o nome |
| `C(p1, campo=p2)` com `C` dataclass | `ref.test $C`, e cada campo casa o seu padrão |
| `int()`, `str()`, `float()`, `bool()` | o tipo do valor uniforme |

- `case 1` casa `1.0` e `True`, porque usa `==`. `case True` não casa
  `1`, porque usa `is`.
- A posição de um subpadrão de classe vem do `__match_args__`, que o `ty`
  dá em `class_info().match_args`.
- Um atributo dado por posição e por nome, como `P(2, x=1)`, lança no
  CPython `TypeError: P() got multiple sub-patterns for attribute 'x'`
  quando o caso é testado. O checker não acusa. O compilador lança o
  mesmo erro ali. Um nome que a classe não tem só faz o caso não casar.
- `int(x)`, `str(x)`, `float(x)` e `bool(x)` com um subpadrão casam o
  próprio valor.
- `case int()` casa um `True`, porque `bool` é subclasse de `int`, e o
  teste de `int` aceita o `$Bool` da forma uniforme.
- Os nomes de um padrão ficam ligados quando o padrão inteiro casa, antes
  da guarda. Uma guarda que falha deixa os nomes ligados, como no
  CPython. Um padrão que falha no meio não liga nada, e o CPython 3.13
  faz o mesmo nos casos testados.
- A captura e o `as` ligam nomes. `Bindings` em `lower/flow.rs` tem de
  contá-los em `assigned` e em `order`, a ordem dos locais do quadro, ou
  a função falha. A captura liga a variável com a `Rep` que o `ty` dá ao
  nome, e desfaz a forma do sujeito quando ela é mais larga.

### O fim de uma função

A análise do texto (`always_returns`) fica como está. O capítulo 05 não
retorna em cada ramo de uma cadeia sobre um Enum. Ele liga uma variável
em cada ramo e retorna depois, e a leitura passa pelo
`is_definitely_bound` do `ty`. Uma função com `return` em cada ramo de
um `if`/`elif` completo continua recusada, e
o plano registra isso em "Em aberto".

## Etapas

Cada linha é um commit, e cada commit passa pelo portão rápido. O Enum
vem antes da dataclass, porque 6 dos 8 arquivos só precisam dele.

**A. Infraestrutura**

1. `compiler: emit the types of a rec group together`
2. `compiler: accept the imports of enum and dataclasses`
3. `compiler: add a table of classes to the IR`
4. `compiler: add Rep::Object and Rep::ObjectOrNone`

**B. O Enum**

5. `compiler: recognize the accepted form of an Enum`
6. `compiler: make the members of an Enum globals of the module`
7. `compiler: bind the name of a class with a flag`
8. `compiler: compile .name and .value of a member`
9. `compiler: compare members of an Enum with ==, != and is`
10. `compiler: generate the repr and the str of a member`
11. `compiler: dispatch a uniform value to the classes by ref.test`

**C. A dataclass**

12. `compiler: recognize the accepted form of a dataclass`
13. `compiler: construct a dataclass from call_bindings`
14. `compiler: evaluate the defaults of a dataclass in the class`
15. `compiler: raise NameError for an annotation without a value`
16. `compiler: read and assign a field of a dataclass`
17. `compiler: generate the repr of a dataclass`
18. `compiler: write ... for a dataclass inside its own repr`
19. `compiler: generate == of a dataclass`
20. `compiler: accept @dataclass(frozen=True)`

**D. Fecho**

21. `compiler: check the chapter 05 of the corpus at level 2`, com
    `("05-tipos-de-dados", Level::UserTypes)` em `CHAPTERS` e `COVERED`
    em `Level::UserTypes`
22. `docs: record the slice 3 in the plan of the backend`

## Testes

- Cada commit traz os testes do que ele compila, em
  `compiler/tests/run/`, com o CPython como oráculo do stdout, do stderr
  e do código de saída, no wasmtime e no deno.
- Cada forma recusada tem um teste de `Unsupported` com a mensagem.
- Casos de borda com o CPython como oráculo:
  - o `auto()` depois de valores que não sobem;
  - o `repr` com ciclo e o `repr` fundo até o `RecursionError`;
  - o `==` com `nan`, com o próprio objeto e com dois ciclos;
  - o default de dataclass e a anotação sem valor no `class`;
  - a leitura de uma variável ligada em cada ramo de uma cadeia que o
    `ty` prova completa, com o aviso `redundant-condition`.
- O portão do corpus sobe para o nível 2 no commit 21.

## Riscos

- **O `ty` e a forma.** `class_info` nunca rodou no compilador. Além do
  `auto()`, o `ty` não vê apelidos entre `1`, `1.0` e `True`. A forma
  aceita recusa esses casos, mas outro desvio do `ty` aparece só nos
  testes.
- **Os quadros gerados.** O traceback do `repr` e do `==` cita linhas de
  código que o CPython gera e o caminho do `reprlib` no Linux. Uma versão
  nova do CPython muda essas linhas.
- **O `ref.test` e a vtable.** Esta fatia decide a classe por `ref.test`,
  e a fatia 6, com herança, troca isso pela `$VT` do plano do backend.
  O despacho fica num lugar só para essa troca mexer num arquivo.

## Em aberto

- Uma função com `return` em cada ramo de um `if`/`elif` que o `ty`
  prova completo. O compilador recusa, e o checker
  aceita. Confiar no `ty` exige `unreachable` no fim, e só quando o
  retorno não aceita `None`.

## O que a revisão mudou

- A motivação do `unreachable` era falsa, e a etapa saiu. O capítulo 05
  liga uma variável em cada ramo, e não retorna em cada um.
- Faltava o import de `enum` e de `dataclasses`.
- A `$VT` e o `$Object` saíram. Dentro de um rec group, dois structs de
  mesma forma são distintos, e o `ref.test` decide a classe.
- Os membros e as classes não cabem em globais imutáveis, porque o
  `$Str` não é constante.
- O Enum veio para antes da dataclass, e o despacho para depois do
  primeiro objeto.
- O `auto()` usa o maior valor anterior. O valor do compilador é
  conferido com o do `ty`.
- O `==` começa por `self is other`. O `repr` e o `==` têm quadros
  próprios no traceback.
- O default de um campo e a anotação sem aspas têm a semântica do
  `class` do CPython.
- A decisão sobre os nomes de um padrão que falhou sumiu, porque o
  CPython 3.13 não os liga.
- `dataclasses.replace` e a cópia de classe nos doctests saíram.

## Decisões

O usuário tomou estas decisões em 10/nov/2026, uma pergunta por vez.

1. O `match` sai desta fatia e vai para depois, porque nenhum arquivo do
   corpus o usa.
2. A fatia aceita `@dataclass(frozen=True)`, que não tem uso no corpus
   mas é comum no ensino, e recusa qualquer outro argumento do
   decorador.
