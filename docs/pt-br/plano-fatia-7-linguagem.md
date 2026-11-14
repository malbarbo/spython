# Plano: a fatia 7, a linguagem completa

Este plano detalha a fatia 7 do [plano do backend
WasmGC](plano-backend-wasmgc.md). A fatia 7 era "Imagens e programas
interativos", e o usuário a mandou para a fase do REPL, a fatia 10, em
13/nov/2026, junto com o `input`, o `...` como comando e o capítulo 09.
No lugar dela
entra a linguagem completa, que é o subconjunto estático do Python 3.13
que o spython aceita no nível 5, com `try` e com o protocolo de iteração.
A fatia 8, "Nível 5 por partes", fica contida nesta, e os inteiros
grandes passam a ser a fatia 8.

## O subconjunto estático

Um programa está no subconjunto quando o checker o aceita no nível 5. O
`dynamic-type` já recusa um valor de tipo `Any` ou `Unknown` (decisão 15
do plano do backend), como o resultado de `eval`, `getattr` e `open`. No
que sobra, o tipo de cada valor é conhecido antes de o programa rodar, e
cada construção tem um significado que o tipo estático decide.

A fatia compila tudo isso, com três exceções. A primeira é o que precisa
de um módulo que o spython não tem, como `asyncio` para `async def`. A
segunda é o que muda o próprio programa enquanto ele roda, como uma
metaclasse ou `__getattr__`. A terceira é a lista da seção "O que fica de
fora", que o usuário decidiu.

O checker ainda aceita boa parte do que fica de fora, como `exec`,
`setattr`, `hasattr`, `dir()`, `1j`, `b"ab"`, `async def`, `metaclass=`,
`__getattr__`, `del c.x`, `except*` e `.send`. A parte 7k dá um
diagnóstico a cada um (decisão 3 do plano do backend). Ela também cobre
as recusas do compilador que não são de uma construção, como "a field
that __init__ may not assign" e "two methods of the same name". No fim da
fatia, o compilador nunca encontra um `Unsupported` num programa que o
checker aceitou.

## Onde a fatia começa

Para medir o que falta, cerca de 290 programas pequenos, um por
construção, rodaram no compilador e no CPython 3.13, com
`PYTHONHASHSEED=0`. Três revisores acrescentaram outros cem. O compilador
já cobre quase todos os métodos de `str` e de `list`, o básico de `dict` e
de `set`, as compreensões, o `lambda`, as closures, a herança com os
métodos especiais, as uniões com `isinstance`, `T | None`, o `sorted` com
`key=`, a atribuição encadeada e o `assert` com mensagem.

O portão do corpus não pegou os buracos dos níveis 0 a 4, porque o corpus
não usa essas construções. A tabela mostra o que o checker aceita e o
compilador recusa, com o primeiro nível em que o checker aceita.

| Construção | Nível |
|---|---|
| `print(..., end="")` e `sep=` | 0 |
| o formato de f-string, como `{x:.2f}`, `{n:03d}`, `{x!r}` e `{x=}` | 0 |
| `str()` e f-string de `None` e de uma união, como `f"{x}"` com `x: int \| None` | 0 |
| uma variável com valores de dois tipos, como `m = None` e depois `m = x` | 0 |
| `s.split(",")`, `split` com `maxsplit`, `rsplit`, `splitlines`, um argumento nomeado num método de `str` | 0 |
| `~n`, `<<`, `>>`, `:=`, `exit()`, `pow` com três argumentos, `bin`, `hex`, `oct` | 0 |
| `List`, `Dict`, `Optional`, `Union`, `Final`, `Literal`, `Self`, `Never`, `cast`, `overload`, `assert_never` de `typing` | 0 |
| a conta com `int \| float`, o tipo que `int ** int` dá (decisão 11) | 0 |
| uma união ou `object` que pode guardar uma lista, como `int \| list[int]` | 0 |
| `divmod`, `partition` e o desempacotamento do resultado | 0 |
| `list("abc")`, `list(range(3))`, `reversed`, `sorted("cba")`, `set(s)`, `dict()`, `max(..., key=len)` | 0 |
| uma função genérica, como `def f[T](x: T) -> T` | 0 |
| `class C():`, `class C(object):` e o apelido `Forma = A \| B` | 2 |
| `@dataclass(order=True)`, `field()`, `replace` | 2 |
| `Cor(1)`, `Cor["A"]`, `len(Cor)`, `IntEnum`, `StrEnum` | 2 |
| uma classe de exceção do aluno e o `raise` dela | 2 |
| a literal de tupla, `for a, b in pares` | 3 |
| `xs[i] += 1`, `d[k] += 1`, `m[i][j] += 1` | 3 |
| `[0] * n`, `xs += ys`, `xs < ys`, a atribuição a uma fatia | 3 |
| `enumerate`, `zip`, `range` como valor, `sum(d.values())`, `sum(xs, 0.0)` | 3 |
| `for c in Cor` | 3 |
| um atributo que só o `__init__` atribui, sem anotação no corpo da classe | 4 |
| um método especial numa dataclass, além de `__str__` e `__repr__`, e `__post_init__` | 4 |
| a herança entre uma dataclass e uma classe comum | 4 |
| `setdefault`, `update`, `popitem` e `fromkeys` de `dict`, `items()` fora do `for` | 4 |
| os métodos de `set`, como `union`, `update`, `issubset` e `pop` | 4 |
| `key=len`, `map(str, xs)`, um método ligado como valor | 4 |
| `@staticmethod`, `@classmethod`, `@property`, um atributo de classe com valor | 4 |
| uma classe genérica, uma classe dentro de função, um default num `def` interno | 4 |
| `__abs__`, `__int__`, `__float__`, `__pos__`, `__invert__`, `__index__`, `__round__`, `__delitem__`, `__reversed__`, `__lshift__` | 4 |
| `try`, `with`, `del`, `nonlocal`, `yield` | 5 |

O plano da fatia 6 diz que o checker exige a anotação no corpo da classe.
Ele exige só para uma atribuição no corpo. Um `self.saldo = saldo` no
`__init__`, sem `saldo: float` no corpo, passa no nível 4, e o compilador
o recusa. É a forma mais comum de escrever uma classe.

Os inteiros de 31 bits também aparecem cedo. `2 ** 31` e `100000 *
100000` no nível 0 e `fat(13)` com um laço no nível 3 param o programa,
onde o CPython escreve o valor.

### Os limites do checker

As sondas acharam programas do subconjunto que o checker recusa:

- Um `lambda` dentro de um contexto de tipo `object`, como o argumento de
  `print`, recebe `object` no parâmetro. `print(max(xs, key=lambda p:
  p.i))` falha e `m = max(xs, key=lambda p: p.i)` passa.
- `filter(lambda x: x > 1, xs)` deixa o parâmetro `Unknown` em qualquer
  contexto.
- Um default num `lambda`, como `lambda x, k=2: x * k`, dá `Unknown |
  Literal[2]` ao parâmetro.
- `return sum(xs)` com `xs: list[float]` e resultado `float` falha,
  porque `sum([])` é o `int` 0. O checker está certo, e falta a sugestão
  de escrever `sum(xs, 0.0)`.
- `e.args` e `StopIteration.value` são `Any`, e o `dynamic-type` os
  recusa.

Os quatro primeiros são programas comuns, e a parte 7d os corrige no fork
antes da iteração. O último fica de fora. O limite do `ty` com os
geradores está na seção "Geradores, para o fim".

## O que a fatia entrega

As partes seguem o nível do checker e as dependências. A tupla vem antes
da iteração, porque `enumerate`, `zip` e `items()` dão tuplas. A iteração
que o prelude faz sem exceção vem antes das exceções, porque o nível 3 a
usa. O `__next__` do aluno vem depois das exceções, porque termina com
`StopIteration`. A função com `yield` fica para o fim do plano do backend,
por decisão do usuário, e a seção "Geradores, para o fim" guarda o que a
análise achou.

**7a. O portão e os buracos dos níveis 0 a 3**

1. As sondas viram um teste do compilador, com o resultado esperado de
   cada uma. Ele falha quando o checker aceita uma sonda que o compilador
   recusa e ela não está na lista do que falta. Cada parte tira itens da
   lista, e a 7k a esvazia.
2. O `print` com `sep`, `end` e `flush` vai para a 7g, no item 44,
   porque é escrito no prelude com `*values`.
3. `split` com separador e `maxsplit`, `rsplit`, `splitlines` e os
   argumentos nomeados dos métodos de `str` e de `list`, pela mesma
   ligação.
4. `~`, `<<`, `>>`, `pow` com três argumentos, `bin`, `oct`, `hex`,
   `ascii`, `int.bit_length`, `int.bit_count`, `float.is_integer`,
   `float.hex`, `float.fromhex`, `conjugate`, `real`, `imag`,
   `numerator` e `denominator`.
5. A conta e a comparação com `int | float`, e o `str()`, o `print` e a
   f-string de `None` e de uma união.
6. Uma lista, um `dict`, um `set` e uma função dentro de uma união ou de
   um `object`, com o `print`, o `==` e o `isinstance`.
7. A variável com valores de dois tipos, que ganha a `Rep` da união que o
   `ty` declara para ela.
8. `:=`, inclusive dentro de uma compreensão, onde o nome é da função em
   volta.
9. De `typing`, os nomes antigos (`List`, `Dict`, `Set`, `Tuple`,
   `Optional`, `Union`), `Final`, `Literal`, `TypeAlias`, `Self`,
   `Never`, `NoReturn`, `TYPE_CHECKING`, `overload`, `override`, `final`,
   `NewType`, `assert_never` e `cast`, que testa o tipo em tempo de
   execução como o `TypeError` do CPython, o comando `type X = ...` e o
   apelido implícito `Forma = A | B`.
10. O operador com atribuição num item, como `xs[i] += 1`, `d[k] += 1`,
    `m[i][j] += 1` e `obj.xs[i] += 1`.
11. `[x] * n` e `n * [x]`, `xs += ys` e `xs *= n` na própria lista, a
    ordem entre listas, `==` e `+` entre `list[int]` e `list[bool]`, e a
    atribuição a uma fatia.
12. `class C():` e `class C(object):`.
13. `sum` de uma lista com `start`.

**7b. Tuplas**

14. A tupla de tamanho fixo, como `tuple[int, str]`, e a de tamanho
    variável, `tuple[int, ...]`, com a literal, o índice constante e o
    variável, `len`, `in`, `count`, `index`, a fatia, `+`, `*`, `==`, a
    ordem, o `hash`, o `print` e o `repr`.
15. O desempacotamento na atribuição e no alvo do `for`, com `*resto`, e a
    troca `a, b = b, a`.
16. A tupla como parâmetro, resultado, item de lista, chave de `dict`,
    item de `set` e chave de `sort`, como em `key=lambda s: (len(s), s)`.
17. `divmod`, `as_integer_ratio`, `partition`, `rpartition`,
    `startswith` e `endswith` com uma tupla, `math.frexp`, `math.modf`,
    `dict.popitem` e `isinstance(x, (A, B))`.
18. `NamedTuple`, uma tupla gerada com nomes nos campos, com o `repr`
    `P(x=1, y=2)` e o desempacotamento, o índice e o `==` da tupla.

**7c. O formato**

19. A minilinguagem de formato do CPython para `int`, `float`, `str`,
    `bool` e um objeto com `__format__`, no formato de f-string, em
    `format()` e em `str.format` com um modelo literal, com os campos
    aninhados, `!r`, `!s`, `!a` e o `=`.
20. O `%` de `str`, com o seu próprio leitor no prelude em Python, que
    serve ao modelo literal e ao que vem de uma variável, e o
    `%(nome)s` sobre um `dict`. Um modelo literal é lido na compilação.

**7d. A iteração do prelude**

21. Os limites do `ty` da seção "Os limites do checker", no fork.
22. O protocolo interno `next() -> bool`, com `item()`, em todo iterador
    do prelude.
23. Em Python no prelude, `enumerate`, `zip`, `map`, `filter`, `reversed`,
    o objeto `range` e as views `keys()`, `values()` e `items()`. O
    `frozenset`, uma cópia do `Set` sem os métodos que mudam o conjunto,
    com o `hash` do `frozenset` do CPython.
24. `Iterable[T]` e `Iterator[T]` como tipo de parâmetro, de variável e de
    resultado, e `Sequence[T]` e `Mapping[K, V]`, pelo nó `Implements` da
    abstração de despacho.
25. Os consumidores de qualquer iterável: `list()`, `tuple()`, `set()`,
    `frozenset()`, `dict()`, `sorted`, `sum`, `min` e `max` com
    `key` e `default`, `any`, `all`, `join`, `in`, os métodos de `dict` e
    de `set` que recebem um iterável, `math.fsum`, `math.prod`,
    `math.sumprod` e `math.dist`.
26. O desempacotamento de um iterável e o `*` e o `**` numa literal e numa
    chamada, como `[*xs, *ys]`, `{**d}`, `{*xs}` e `f(*xs)`.
27. `iter` e `next` sobre os iteradores do prelude.

**7e. `match`**

28. Os padrões de literal, de captura, `_`, de valor (um membro de Enum
    ou uma constante com ponto), `|`, `as` e a guarda.
29. O padrão de classe, posicional pelo `__match_args__` e por nome, com
    uma dataclass, uma classe do aluno e os tipos builtin, como `int(n)`.
30. O padrão de sequência sobre lista, tupla e `range`, com `*resto`.
31. O padrão de mapeamento sobre `dict`, com `**resto`.

**7f. Classes**

32. A abstração de despacho, antes de qualquer construção nova. A IR
    ganha os nós `Dispatch`, `Implements` e `ClassOf`, ao lado do
    `IsInstance`, e um módulo do emissor escolhe o mecanismo. A primeira
    versão usa só a cadeia de `ref.test` da fatia 6, sem mudar nenhuma
    saída, com o portão da suíte inteira. Depois entra o id de classe,
    com o `br_table` e o intervalo, escolhido pelo limite da sondagem
    S3. O descritor `$Class` de cada classe, com o nome qualificado, que
    `type(x)`, o `print` de uma classe, as mensagens de `NotImplemented` e
    as exceções usam.
33. O atributo que só o `__init__` atribui, e o atributo comum com valor
    no corpo, como `saldo: float = 0.0`, que inicia o campo.
34. O atributo de classe com `ClassVar`, `@staticmethod`, `@classmethod` e
    `@property`, com o setter.
35. `@dataclass` com `order`, `eq`, `frozen`, `unsafe_hash` e `kw_only`,
    `field()` com `default`, `default_factory`, `init`, `repr`, `compare`
    e `kw_only`, `KW_ONLY`, `InitVar`, `__post_init__`,
    `dataclasses.replace`, os métodos especiais numa dataclass e a herança
    entre uma dataclass e uma classe comum.
36. O Enum como valor, com `for c in Cor`, `Cor(1)`, `Cor["A"]`,
    `len(Cor)`, `x in Cor`, os apelidos, `@unique`, `IntEnum`, `StrEnum`,
    um membro com valor de tupla e `__init__`, e `Flag` e `IntFlag`, com
    as partes do `enum.py` que o programa usa traduzidas para o prelude
    em Python: as operações de bits, o `repr` como `<Perm.R|W: 6>`, a
    iteração sobre os membros de uma combinação e o `in`. O `TypedDict`,
    que em tempo de execução é um `dict` com chaves `str` e valores na
    forma uniforme, e cuja leitura usa o tipo que o `ty` dá à chave.
37. `type(x)`, `type(x).__name__`, `type(x) is C`, `issubclass`, o
    `print` de uma classe e a classe como valor geral, com `type[C]`, a
    chamada, o atributo de classe e o `isinstance` com a classe vinda de
    uma variável.
38. `__call__`, `__abs__`, `__int__`, `__float__`, `__pos__`,
    `__invert__`, `__index__`, `__round__`, `__delitem__`, `__reversed__`,
    `__lshift__`, `__rshift__` e os refletidos, o `hash` e o `id` de
    identidade para uma classe sem `__eq__`, e `NotImplemented` nos
    métodos de operador.
39. A classe genérica do aluno, com `class C[T]`, `TypeVar` e `Generic`.
40. A classe dentro de função e a classe dentro de classe.
41. O `...` como corpo de um método, de um `Protocol`, de um `overload` e
    de um método abstrato. O módulo `abc`, com `ABC`, `abstractmethod` e o
    `TypeError: Can't instantiate abstract class Forma without an
    implementation for abstract method 'area'`, e o `from __future__
    import annotations`, sem as outras opções do `__future__`. Os dois
    módulos entram no `AVAILABLE_MODULES` de `engine/src/checker.rs`.
42. A herança múltipla em que só uma base tem campos, pelo nó
    `Implements`, com a ordem C3 que o `ty` calcula.

**7g. Funções**

43. Um builtin, um método e um método ligado como valor, como `key=len`,
    `str.lower` e `f = obj.m`, e um valor de função cujos parâmetros têm
    outra `Rep`.
44. `*args`, `**kwargs`, o `/`, a chamada com `*xs` e `**d`, os
    decoradores de função, o default num `lambda` e num `def` interno,
    tudo pela ligação de argumentos da seção "Argumentos nomeados", no
    aluno e no prelude, com a cópia por forma da chamada da seção
    "Funções com `*args`". O `print` com `sep`, `end`, `flush` e
    `sep=None` e `end=None` vem em seguida, escrito no prelude, e o portão
    é o tempo do `print` de hoje, medido nos dois modos da opção de
    cópia.
45. `nonlocal`.

**7h. Exceções**

A parte tem quatro etapas, cada uma com o seu portão.

46. As exceções viram classes do prelude, sem sintaxe nova. O portão é o
    traceback e o relatório dos doctests iguais, byte a byte, aos de
    hoje.
47. `try` com `except`, `except E as e`, `except (A, B)`, `except:` e
    `else`, `raise` de uma classe e de um objeto e o `raise` sozinho, a
    classe de exceção do aluno com campos, métodos e `__str__`, a
    exceção como valor e o `RecursionError` capturável.
48. `finally`, com `return`, `break` e `continue` dentro do `try` e do
    `finally`, `with` com várias expressões e entre parênteses, `del` de
    um nome, de um item e de uma fatia, e `exit()` e `quit()` com
    `SystemExit`.
49. `raise ... from ...`, `from None`, o `__context__` de toda exceção
    lançada e as duas mensagens de encadeamento do traceback.

**7i. O iterador do aluno**

50. O `__iter__` e o `__next__` do aluno, `iter` e `next` sobre eles, e
    os `Protocol` do aluno pelo mecanismo dos tipos estruturais.

**7j. Os módulos do programa**

51. `import m`, `from m import f`, `as`, `import` dentro de função, o
    pacote com `__init__.py` e o import relativo, com o programa inteiro
    num módulo wasm.
52. `__name__` e o `if __name__ == "__main__"`, que o módulo importado usa.
53. O corpo de cada módulo roda no primeiro import, na ordem do CPython,
    com a mensagem do import circular.
54. O traceback com o arquivo de cada módulo, e o nome qualificado de uma
    exceção de outro módulo, como `m.Erro: x`.

**7k. O checker**

55. Um diagnóstico para cada construção que fica de fora, e para cada
    recusa do compilador que não é de uma construção.
56. A lista do teste do item 1 vazia.

## O que fica de fora

| Item | Por quê |
|---|---|
| `eval`, `exec`, `compile`, `getattr`, `setattr`, `hasattr`, `delattr`, `globals`, `locals`, `vars`, `dir`, `__import__`, `type` com três argumentos, `__dict__` | o tipo do resultado é dinâmico, ou o resultado depende de um nome dado em tempo de execução |
| `str.format` com modelo que não é literal, com `.` ou `[]` num campo, e `format_map` | ler um atributo pelo nome é `getattr` |
| `async`, `await`, `async for`, `async with` | o spython não tem `asyncio` nem outro laço de eventos |
| `open` e os arquivos | o resultado é dinâmico, e o navegador não tem arquivos |
| `complex` | decisão 11 do plano do backend |
| `bytes`, `bytearray`, `memoryview`, `str.encode`, `int.to_bytes`, `int.from_bytes` | pedem um tipo novo, sem uso para o aluno |
| metaclasse, `__new__`, `__init_subclass__`, `__getattr__`, `__getattribute__`, `__setattr__`, `__delattr__`, descritor além de `property`, `__slots__`, `__del__` | mudam o significado de um atributo ou da criação de um objeto enquanto o programa roda |
| `del obj.attr`, e a escrita pela classe num atributo sem `ClassVar`, como `C.n = 7` | o campo do struct existe sempre |
| um campo que a subclasse redeclara com outro tipo | um campo mutável não muda de tipo num subtipo |
| `super(C, self)` com argumentos | `super()` cobre o caso comum |
| `except*`, `ExceptionGroup`, `add_note` | sem uso para o aluno |
| `e.args`, `StopIteration.value`, `asdict`, `astuple`, `fields` | o `ty` lhes dá um tipo com `Any`, e o `dynamic-type` os recusa |
| a função com `yield`, `yield from` e a expressão geradora como valor | ficam para o fim do plano do backend, na seção "Geradores, para o fim" |
| a herança múltipla com campos em mais de uma base | um struct do WasmGC tem um supertipo só |

## Desenho

### Tuplas

Uma tupla de tamanho fixo é uma classe gerada pelo compilador, uma por
lista de `Rep`, com campos imutáveis. Assim ela é um `Rep::Object` como
outro, e ganha de graça o `ObjectOrNone`, a criação, a leitura de campo, o
`isinstance` e o lugar no despacho uniforme do `print`, do `==` e do
`hash`. O `repr`, o `==`, a ordem e o `hash` são funções geradas, como as
da dataclass, porque o prelude em Python não escreve uma função para um
número qualquer de campos. O `hash` segue o `tuplehash` do CPython 3.13,
que a IR já tem em `HashTuple`. A chave da classe gerada é a própria
lista de `Rep`, e não um nome impresso. O backend do Gleam guardava as
funções geradas pelo nome impresso do tipo e juntou dois `Color` de
módulos diferentes.

A ordem acha o primeiro item diferente pela identidade e depois pelo
`==`, como o `PyObject_RichCompareBool`, e só então compara com `<`. A
lista do prelude já faz assim. Com `n = math.nan`, `(n, 1) < (n, 2)` dá
`True` no CPython, e o teste de `nan` usa dois `float("nan")`.

Uma tupla de tamanho variável, `tuple[T, ...]`, é um array imutável no
rec group, onde ele não se confunde com o `str`. A coerção de uma tupla
fixa para uma variável, como de `tuple[int, int]` para `tuple[int, ...]`,
copia os itens, e a sondagem S2 mede se uma tupla fixa de itens iguais
deveria ser o array.

O índice variável numa tupla fixa chama uma função gerada que escolhe o
campo, com a `Rep` da união dos itens. O desempacotamento avalia o lado
direito inteiro e depois atribui os alvos da esquerda para a direita, e
assim `i, xs[i] = 2, 9` escreve em `xs[2]`. O de um iterador consome um
item além do esperado para dar o erro. As mensagens são as do 3.13, como
`too many values to unpack (expected 2)`, sem o "got".

### O formato

Um formato constante, como `.2f`, é lido pelo compilador, e a chamada vai
direto para a função do prelude com os campos já separados. Um formato que
depende de um campo aninhado, como `{x:{w}}`, é lido em tempo de execução,
com os erros do CPython. O alinhamento, o preenchimento, o sinal, o
agrupamento e as bases de `int` ficam em Python. Os dígitos de `float` com
precisão vêm de `exact()`, no `round.rs` do crate de builtins, que já
arredonda o empate para o par como o `PyOS_double_to_string`.

Os casos que o teste cobre são do CPython. Um `bool` com formato não
vazio se formata como `int`. Um formato vazio chama `__format__('')`, e
não `__str__`. `f"{x=}"` usa o `repr`, e `f"{x=:5}"` usa o formato. Um
objeto sem `__format__` com formato não vazio lança `TypeError`.

O `%` de `str` tem regras próprias. `"%d" % 3.7` dá `'3'`, enquanto
`format(3.7, "d")` lança `ValueError`, e um valor `object` que guarda uma
tupla é desempacotado. Por isso o `%` tem o seu leitor e as suas
conversões, e divide com o formato só os dígitos. O leitor fica no
prelude, em Python. Com um modelo literal, o compilador lê o modelo e
chama as conversões direto, com o mesmo resultado. As mensagens são as do
CPython, como `not enough arguments for format string` e `not all
arguments converted during string formatting`.

### A iteração do prelude

O prelude já itera um `dict` por um protocolo interno: `DictIter.next()`
devolve `bool` e `key()` e `value()` leem o item. A proposta generaliza
esse protocolo. Todo iterador do prelude tem `next() -> bool` e
`item() -> T`, e o `for` chama os dois sem exceção nenhuma. O `__next__`
público de um iterador do prelude é escrito em Python sobre os dois, e
lança `StopIteration` no fim.

O `next()` de `map` e de `filter` chama código do aluno. Um
`StopIteration` lançado pela função do aluno termina o laço em silêncio no
CPython, e por isso esse `next()` o captura e devolve `False`. Até a parte
7h, ninguém lança `StopIteration`.

O adaptador do iterador do aluno é um `try` com `except StopIteration`
do prelude, que devolve `False` de dentro do `except`. Esse `return`
restaura o global da exceção tratada, como toda saída de uma função com
`try`. Sem isso, uma exceção lançada depois do laço teria o
`StopIteration` como `__context__`, e o traceback o mostraria com
"During handling of the above exception", o que o CPython não faz.

O `for` sobre um tipo concreto não muda. Uma lista continua com o índice,
um `str` e um `range` também, e um `enumerate(xs)` chama direto o `next()`
da classe `enumerate` do prelude. O `zip` para no primeiro argumento que
termina, sem avançar os seguintes, e o item já tirado dos anteriores se
perde. As views são vivas, e a mudança do `dict` durante a iteração por
uma view dá as mesmas mensagens do `DictIter`.

### Os tipos estruturais

`Iterable[T]`, `Iterator[T]`, `Sequence[T]`, `Mapping[K, V]`, um
`Protocol` do aluno e uma base sem campos não são o tipo wasm dos valores
que os satisfazem, porque um struct do WasmGC tem um supertipo só, e a
base com campos o ocupa. Um valor deles tem a forma uniforme. A chamada
de um método dele é um `Dispatch`, e o `isinstance` é um `Implements`, os
dois da abstração de despacho da seção "Classes".

O compilador vê o programa inteiro e sabe que classes satisfazem cada
`Protocol` e cada `Iterable[T]`, mesmo sem declaração, e essa lista não
sai das coerções, porque `Iterable[int]`, `object` e `int | str` têm a
mesma `Rep::Uniform`. Ela cobre cada classe, cada cópia do prelude e cada
tupla. Com o id de classe, o `Implements` é um bit numa tabela
constante indexada pelo id, e a chamada é um `br_table` sobre o id. Os
dois custam o mesmo com qualquer número de classes. A ordem em pré-ordem
não serve aqui, porque a herança múltipla forma um grafo. Os valores sem
id, que são o `str`, os números e as listas, passam por testes antes. As
listas são um tipo wasm por item, e esse teste cresce com o programa. Se
a sondagem S3 mostrar que ele pesa, a lista ganha o id também. Um tipo
cujo item tem outra `Rep`, como uma `list[int]` num `Iterable[int |
None]`, ganha uma adaptadora que alarga o item.

Um passo sobre um `Iterator[T]` faz um despacho só, que devolve o `bool`
e o item juntos.

### Classes

A lowering decide que corpos são possíveis em cada ponto, pelo RTA, pela
busca de métodos na hierarquia, pela ordem C3 e pelas cópias por classe.
Ela não decide como o programa escolhe o corpo em tempo de execução. A IR
diz a intenção com quatro nós:

- `Dispatch { objeto, braços }` chama o corpo da classe exata do objeto,
  e serve à chamada de método e ao `print`, ao `repr`, ao `==` e ao
  `hash` de um valor uniforme;
- `IsInstance { valor, classe }`, que já existe, serve ao `isinstance` e
  ao `except`;
- `Implements { valor, tipo }` serve a um `Protocol`, a `Iterable`, a
  `Sequence`, a `Mapping` e à base sem campos;
- `ClassOf { valor }` serve a `type(x)`.

Um módulo do emissor, `emit/dispatch.rs`, escolhe o mecanismo de cada
ponto, junto com o cabeçalho do objeto em `emit/layout.rs`. Com um braço,
a chamada é direta. Com poucos, é a cadeia de `ref.test` contra os tipos
exatos `final` da fatia 6. Com muitos, é o id de classe. O limite é uma
constante que a sondagem S3 mede. Hoje o mecanismo está em quatro
lugares, o `dispatch_body` da lowering, o `test_class`, o
`add_uniform_dispatch` e o `add_uniform_eq` do emissor, e depois da
mudança ele fica num só. Trocar de mecanismo mexe nesse módulo e no
cabeçalho, sem tocar na lowering nem na IR.

O id de classe é um campo `i32` imutável na raiz, numerado em pré-ordem
na árvore das bases com campos, e só existe quando algum ponto do
programa o usa. O `Dispatch` com muitos braços é um `br_table` sobre o id
que leva a chamadas diretas. O `IsInstance` é o intervalo `lo <= id <=
hi` da classe. O `ClassOf` lê um array global de `$Class` pelo id. O
plano da fatia 6 refez a sondagem `dispatch.wat` do jwonder no deno, sem
o `wasm-opt`: o id com `br_table` empatou com a cadeia até 16 corpos, e a
vtable perdeu em todos os casos, de 15% a 50%, porque o V8 só põe em
linha um `call_ref` quase monomórfico. O teste contra um tipo sem `final`
custou o dobro, e por isso nenhum mecanismo o usa. A vtable fica como
alternativa que o módulo pode adotar se uma medida futura a favorecer.

Cada classe ganha o descritor `$Class`, um global imutável com o nome
qualificado, a base e o intervalo do id. `type(x) is C` vira um `ref.eq`
contra o global de `C`. O jwonder guarda o objeto da classe do mesmo
jeito. Uma classe local tem o
nome `f.<locals>.Local`, como o CPython escreve no traceback e no
`print`.

A cópia do método por classe instanciada, que o `cls` e o `super()`
pedem, é o braço dessa classe no `Dispatch`, e o `super()` dentro de uma
base sem campos segue a ordem C3 da classe exata.

A classe como valor é a referência ao seu `$Class`. O `$Class` tem uma
entrada para o construtor, com a assinatura do `__init__` da classe do
tipo estático. Uma subclasse com outro `__init__` ganha uma adaptadora,
que chama o `__init__` dela quando ele aceita os argumentos pelas regras
do Python, e lança o `TypeError` do CPython quando não aceita. O
`isinstance` contra uma classe vinda de uma variável compara o id do
objeto com o intervalo que o `$Class` guarda, em tempo constante. O
jwonder mediu como gargalo a alternativa de subir pelas bases. O hash de uma
classe é o de identidade, e a ordem de um `set` de classes entra nas
divergências declaradas.

Um atributo sem anotação no corpo vira campo com o tipo que o `ty` lhe dá
em `instance_attributes`. Um atributo comum com valor no corpo, como
`saldo: float = 0.0`, é para o `ty` um atributo de instância com default.
Ele vira campo, iniciado com o valor antes do `__init__`. A análise de
atribuição definida da fatia 6 vale para os dois.

Um atributo com `ClassVar` vira um global por classe que o define. A
leitura pela instância ou por `self` despacha pela classe exata, porque
uma subclasse pode redefinir o valor. O mesmo vale para um
`@classmethod`. A chamada `C.m()` usa a cópia de `C`, e a chamada por uma
instância ou por `self` despacha pela classe exata do objeto, com uma
cópia do método por classe instanciada.

Um método de operador que pode devolver `NotImplemented` ganha um
resultado uniforme, com um singleton para `NotImplemented`. O ponto de
chamada testa o singleton e segue as regras do CPython. O refletido de
uma subclasse vem antes, o `!=` sai do `__eq__`, o `==` cai na identidade,
e o `__iadd__` cai no `__add__`.

O `hash`, o `id` e o endereço do `repr` padrão vêm de um campo `i32`
de identidade, como o `$monitor` do jwonder. O campo existe só nas
classes cujos objetos o programa pode pôr num `set` ou usar como chave
sem `__hash__` próprio, passar para `id` ou escrever pelo `repr` padrão.
Um valor uniforme conta para toda classe instanciada que chega a essa
forma, e assim a análise erra só para mais. O campo começa com 0 e recebe
o número seguinte de um contador global no primeiro uso, e criar o objeto
custa só gravar o 0. Uma função do módulo é um global constante e tem o
número fixo na compilação, e só uma `lambda` ou uma função local leva o
campo.

O `print` escreve o endereço como o CPython, em `<__main__.C object at
0x7f0000000010>`, `<function f at 0x...>`, `<function f.<locals>.g at
0x...>` e `<bound method C.m of <__main__.C object at 0x...>>`, e
`hex(id(c))` dá o mesmo número que o `print(c)`. O número difere do
CPython, que também não o fixa entre duas execuções, e os testes trocam
`0x[0-9a-f]+` por um marcador nas duas saídas. A ordem de um `set` de
objetos com hash de identidade é a mesma em quase toda execução do
CPython e outra no spython, e entra na lista das divergências
declaradas.

Uma dataclass compara pela identidade, depois pela classe exata e depois
campo a campo, e a ordem entre duas classes diferentes lança `TypeError`.
O Enum segue o CPython no `hash` de um membro, que é o `hash` do nome, e
no `IntEnum`, que é o do valor. As mensagens de `Cor(5)`, `Cor["Z"]` e do
campo `frozen` mostram quadros do `enum.py` e do `dataclasses.py`, que o
compilador imita como o `cpython/dataclasses.rs` já faz.

### Funções como valores

Um builtin, um método de `str` e um método ligado viram valor por uma
função adaptadora, gerada por uso, que tem a assinatura do tipo de
função esperado. A mesma adaptadora converte os parâmetros quando a `Rep`
difere, como `def f(x: object)` onde se espera `Callable[[int], int]`.

### Argumentos nomeados

Uma chamada tem um único caminho, seja ela de uma função do aluno, de
uma função do prelude ou de um método de `str` e de `list`. O `ty` liga
cada argumento a um parâmetro da assinatura que ele conhece, a do `def`
do aluno ou a do typeshed, e o compilador usa essa ligação para pôr os
argumentos na ordem dos parâmetros, completar os defaults e manter a
ordem de avaliação. A função do aluno já é chamada assim. O método do
prelude passa a ser chamado assim também, e por isso o `def` do prelude
tem os mesmos nomes de parâmetro que o typeshed. Um teste compara as
duas assinaturas. O parâmetro só posicional do typeshed fica certo sem
código a mais, porque o checker recusa `s.count("a", start=1)` como o
CPython recusa.

O builtin que dá para escrever em Python, como `print`, `sorted`, `min`
e `max`, vai para o prelude e ganha o mesmo caminho. Quanto menos o
compilador souber de um builtin, melhor, porque o prelude passa a
exercitar o que o programa do aluno também usa. O que fica em
Rust, como `int(x, base=)` e `math.isclose`, lê os argumentos da mesma
ligação do `ty`, e não compara nomes no AST.

### Funções com `*args`

Uma função com `*args: T` recebe uma tupla, e um `T` como `object` pede
uma caixa para cada valor e a escolha do método pelo tipo em tempo de
execução. Quando a chamada tem o número de argumentos e a `Rep` de cada
um conhecidos, o compilador faz uma cópia da função para essa forma, como
já faz com as classes genéricas do prelude. Na cópia, o `*args` vira
parâmetros, a caixa some e o despacho fica estático. A cópia vale para
toda função com `*args`, do aluno e do prelude. A chamada com `*xs` de
tamanho desconhecido usa a função sem cópia.

A cópia é uma opção do compilador, ligada por padrão, e a outra escolha
é o despacho dinâmico, sempre pela função sem cópia. `compiler::compile`
recebe as opções numa estrutura, e a CLI não mostra a escolha ao aluno.
A cópia deixa o código mais rápido e o módulo maior, e o despacho
dinâmico deixa o módulo menor e a compilação mais rápida, o que pode
servir ao REPL e ao navegador. As sondas e o corpus rodam nos dois modos
e têm de dar a mesma saída, e assim a função sem cópia continua testada
mesmo quando quase nenhuma chamada a usa. A opção cobre só as funções
com `*args`. As cópias por tipo das classes genéricas do prelude ficam
fora, porque elas não têm uma versão uniforme.

O `print` do prelude é o caso que mede a cópia. Ele converte cada valor
para `str` e escreve a parte, o `sep` e o `end` por uma intrínseca, como o
`builtin_print` do CPython faz com o `write` do arquivo. O `file=` com um
objeto que tem `write` entra quando houver um destino além da saída
padrão, e não muda o compilador.

### Exceções como objetos

Hoje a exceção é um `$Exc` com a classe, um `str` opcional e o
traceback, e a classe é um `$Class` com o nome e a base, montado da
tabela de `cpython/exception.rs`. O `raise` aceita só uma classe builtin,
com no máximo um argumento.

`BaseException` e a hierarquia builtin viram classes do prelude, em
Python, num `exceptions.py`. `BaseException` guarda `args`, o traceback,
`__cause__`, `__context__` e `__suppress_context__`, e define `__str__` e
`__repr__` como o CPython. O descritor `$Class` da parte 7f fica no
prefixo. A classe do aluno estende uma delas como estende qualquer
classe.

O runtime em Rust lança umas 25 classes builtin, como `IndexError` e a
família de `OSError` do `print`, sem passar pela lowering. Por isso o
`Program` leva o `ClassId` e o construtor de cada classe que o emissor
pode lançar, como o `set_class_strs` já leva funções, e essas classes
contam sempre como instanciadas. Sem isso, um `except Exception as e:
print(e)` sobre um `ZeroDivisionError` cairia num slot `unreachable`. A
lista dessas classes sai da mesma tabela que o emissor usa para lançar, e
não de uma segunda lista, porque uma lista fora de sincronia dá um trap e
não só um módulo maior. O jwonder semeia a alcançabilidade assim. O
escritor do traceback e o `caught` dos doctests chamam uma função do
prelude, `last_line(e)`, que escreve o nome qualificado e o `str(e)`, com
o `<exception str() failed>` do CPython.

O `New` de uma classe de exceção preenche `args` com os argumentos
posicionais da chamada, antes do `__init__`, como o `BaseException_new`
do CPython. Assim `E(5)` com um `__init__` que não chama
`super().__init__` tem `args == (5,)`, e `E(x=1)` tem `args == ()`.

O `except E` é o mesmo `IsInstance` de uma classe do aluno, que o
módulo de despacho resolve pela cadeia dos tipos exatos ou pelo intervalo
do id, sem percorrer as bases dos descritores. O jwonder mediu esse
percurso como um gargalo. O `except:` testa `BaseException`, e assim o
Stop e o estouro de 31 bits continuam fora do alcance dele.

A tag continua uma só, com a exceção como carga, porque uma tag não tem
subtipos e não responderia a `except Exception`. Relançar é lançar o
mesmo objeto de novo na mesma tag, sem `exnref` e sem `throw_ref`.

O `try` vira um `try_table` que captura a tag das exceções. O quadro da
função que captura entra no traceback na captura, como no CPython, e a
exceção fica marcada como já tendo esse quadro. Um `raise` sozinho, um
`except` que não casa e um `finally` que relança não acrescentam o quadro
de novo. Um `raise e` acrescenta, e assim o traceback guardado cresce a
cada `raise e`, como no CPython.

O handler testa as classes de cada `except` em ordem, liga o nome do
`as`, roda o corpo e apaga o nome no fim. Uma leitura depois dá o
`NameError` no módulo e o `UnboundLocalError` na função. O `finally` tem
uma cópia só do corpo. Cada saída do `try`, que pode ser o fim normal, um
`return`, um `break`, um `continue` ou uma exceção, guarda o seu número
num local e salta para o `finally`, e depois dele um `br_table` continua
pela saída guardada. O valor do `return` é avaliado antes do `finally`.
Um `return`, um `break` ou um `continue` dentro do `finally` troca a
saída guardada e descarta a exceção. O `leave` de um `return` vai para
depois do `finally`, para que um `finally` que lança não tire 1 do
contador de profundidade duas vezes.

O contador de profundidade já volta ao valor certo na captura, porque o
`reraise` tira 1 por quadro desenrolado. A pilha do `DuringRepr` não
volta, e o `try` guarda o topo dela na entrada e o restaura na captura.
Sem isso, um `__repr__` que lança e é capturado faria o `repr` seguinte
escrever `...`.

O `__context__` vem de um global com a exceção que um `except` ou um
`finally` está tratando. Todo lançamento o preenche, inclusive o do
runtime em Rust e o de uma função chamada que não tem `try`, com o corte
de ciclo do CPython. Um `raise` sozinho sem exceção tratada lança
`RuntimeError: No active exception to reraise`. Só a função com `try`
salva e restaura o global. O escritor do traceback percorre `__cause__` e
`__context__`, com as duas mensagens do CPython.

O `with` chama `__enter__` e `__exit__` como um `try` com
`finally`, com três diferenças. Um resultado verdadeiro de `__exit__`
suprime a exceção. Uma exceção dentro de `__exit__` tem a do corpo como
`__context__`. E o quadro de `__exit__` aponta para a linha do `with`,
com as marcas sob a expressão do contexto. O aluno anota os parâmetros de
`__exit__` com `object`, porque o spython não tem o módulo `types`. O
`with a, b:` e a forma entre parênteses encaixam um `with` no outro, como
no CPython. O `contextlib` fica para quando os geradores vierem, porque o
`@contextmanager` é um gerador.

`exit()` lança `SystemExit(None)`, que termina o programa com 0.
`exit(n)` termina com `n % 256`, e `exit("msg")` escreve a mensagem no
stderr e termina com 1. `except Exception` não captura o `SystemExit`, e
o `finally` roda.

Com o `except`, uma exceção no meio de uma mudança do `sort.py`, do
`set.py` ou do `dict.py`, lançada por um `__eq__` ou por uma chave do
aluno, passa a ser visível. A etapa 47 revê esses três arquivos.

### `match`

O `match` vira uma cadeia de testes, sem tabela. O sujeito é avaliado uma
vez. O padrão de classe usa o `isinstance` e o `__match_args__` que o
`class_info` do fork já dá, sem os campos `kw_only` e `init=False`. O
padrão de sequência aceita lista, tupla e `range`, e não `str`, testa o
tamanho e lê os itens. O padrão de mapeamento testa `in` e lê os valores,
e `case {}` casa com qualquer `dict`. O padrão de valor usa `==`, e
`None`, `True` e `False` usam `is`. Por isso `case 1` casa com `True` e
com `1.0`, e `case float()` não casa com um `int`.

Um `case A() | B():` gera o corpo uma vez só, e cada alternativa salta
para ele. O backend do Gleam repete o corpo para cada alternativa, e a
revisão dele aponta o crescimento do módulo. Um padrão que falha no meio
não liga nenhum nome. Os nomes de um padrão
que casa são ligados antes da guarda. As mensagens de erro do padrão de
classe são as do CPython, como `P() accepts 1 positional sub-pattern (2
given)`. O tipo de cada nome dentro de um `case` vem do estreitamento do
`ty`.

### Os módulos do programa

O programa inteiro continua num módulo wasm até o REPL. Cada módulo
Python tem os seus globais, o seu `__name__` e uma função com o corpo, que
roda no primeiro import e marca o módulo como carregado. As classes e as
funções já são identificadas pelo arquivo e pelo intervalo, e isso não
muda. O `__name__` sai da fase do REPL e entra aqui, porque um módulo
importado usa o `if __name__ == "__main__"`.

## Sondagens

Quatro medidas decidem o desenho antes do código.

- **S1.** O custo de lançar e capturar uma exceção no V8, por laço,
  contra o `next() -> bool`. Ela decide se o iterador do aluno pode
  terminar com o `StopIteration` lançado pelo adaptador.
- **S2.** O tamanho do módulo e o tempo de uma tupla fixa como classe
  gerada, contra um array, num programa com dez formas de tupla.
- **S3.** O tempo de uma chamada de método e de um `isinstance` no V8
  pela cadeia e pelo id, com 16, 32, 64 e 128 corpos, além dos até 16
  que o plano da fatia 6 mediu, e o de um `Implements` sobre uma lista.
  Ela decide o limite entre a cadeia e o id e se a lista ganha o id.
- **S4.** A profundidade que a pilha do V8 aguenta no navegador, com o
  quadro de uma função com trinta locais, para saber se 1000 quadros
  cabem e o `RecursionError` pode ser capturado lá. O sgleam mediu que o
  Node esgota a pilha entre 8000 e 9000 chamadas de uma função pequena.
  O limite dele conta chamadas e o motor conta bytes, e por isso uma
  função com muitos locais estoura antes do limite.

O laço medido em S1 e S3 fica numa função chamada várias vezes, e não no
corpo do módulo. O primeiro nível do V8 não troca para o código otimizado
no meio de um laço, e o jwonder mediu que um laço no `main` mede só esse
nível.

## Testes

- Cada parte traz os testes do que compila, em `compiler/tests/run/`, com
  o CPython como oráculo do stdout, do stderr e do código de saída, no
  wasmtime e no deno.
- As sondas entram como teste no item 1, com um prefixo no nome de cada
  arquivo, porque um arquivo chamado `abc.py` esconde o módulo do
  typeshed dos outros. Quatro sondas desta análise testam outra coisa e
  são corrigidas antes: `protocol`, `exit_builtin`, `e_math_sqrt` e
  `exc_during_handling`.
- Cada divergência declarada tem um teste com uma frase que a explica. O
  teste falha se a saída passar a ser igual à do CPython, e assim a lista
  não guarda uma divergência que já acabou. O jwonder faz assim.
- Antes de comparar, o teste troca cada endereço `0x[0-9a-f]+` por um
  marcador, na saída do spython e na do CPython.
- Casos de borda com o CPython como oráculo:
  - o traceback de uma exceção capturada e lançada de novo em outra
    função com `raise e`, de uma devolvida por uma função e lançada por
    outra, de um `raise e` dentro do próprio `try`, e de um `finally` que
    roda código e relança;
  - `return` dentro de `finally`, `break` e `continue` dentro de `try` e
    de `finally`, um `return x` com `x` mudado no `finally`, e um `try`
    dentro de um `except`;
  - a exceção do aluno com `__init__` que não chama `super().__init__`,
    com argumento nomeado, e o `str` e o `repr` de `KeyError` e de
    exceções com zero, um e dois argumentos;
  - o nome do `as` lido depois do `except`, no módulo e na função;
  - um `IndexError` do runtime dentro de um `except`, e um `raise`
    sozinho numa função chamada pelo `except`;
  - um `RecursionError` capturado e a recursão de novo;
  - um `__repr__` que lança dentro de um `print` de lista, capturado;
  - `exit()`, `exit(256)`, `exit(-1)` e `exit("msg")`;
  - o desempacotamento com itens de menos e de mais, de um iterador do
    aluno, e com `*` no meio;
  - a ordem e o `hash` de tuplas como chaves de `set`, na ordem do
    CPython;
  - `xs[5] += f()` com o índice fora, que não chama `f`, e as marcas sob
    o subscrito;
  - `[[0] * 2] * 2` com a mudança de uma linha, `[0] * -3`, `a += a` e
    `a[1:3] = a`;
  - `map` com uma função que lança `StopIteration`, e `zip` com o primeiro
    argumento menor;
  - um `for` sobre um iterador do aluno que termina, seguido de uma
    exceção fora de qualquer `try`, cujo traceback não mostra o
    `StopIteration`;
  - a mudança de um `dict` durante a iteração por uma view;
  - os empates do formato de `float`, como `f"{0.125:.2f}"`, o
    agrupamento com `,` e `_`, `format(5, "<05")`, `f"{True:>5}"` e
    `f"{x=}"` com espaços;
  - um `match` com padrão de classe de uma subclasse, uma guarda que
    falha depois de ligar um nome, `case 1` com `True` e uma sequência
    dentro de uma classe;
  - um `classmethod` chamado por uma instância de subclasse, e um
    atributo de classe redefinido na subclasse e lido num método da base;
  - um Enum com apelido, o `print` da classe e um `set` de membros.

## Riscos

- **O desenrolar das exceções.** Um `finally` com saídas encaixadas, um
  `return` dentro de um `finally` e uma exceção dentro de um `except`
  mexem com o local da saída, o global da exceção tratada, a marca do
  quadro, a pilha do `repr` e o contador de profundidade ao mesmo tempo.
  Um erro aí aparece só no traceback.
- **A abstração de despacho.** Ela muda o despacho que a fatia 6
  construiu no `resolve_slots`, de que o `print`, o `==`, o `hash` e as
  cópias do prelude dependem. Ela vem antes de qualquer construção nova
  da 7f, só com a cadeia, e com o portão dos testes da fatia 6.
- **O tamanho do módulo.** Cada forma de tupla, cada tabela do
  `Implements`, cada adaptadora e as 25 classes de exceção do runtime geram
  código. A sondagem S2 mede a tupla.
- **A troca do `$Exc`.** O runtime, os doctests e o traceback leem o
  `$Exc` de hoje. A etapa 46 troca isso sem sintaxe nova, com o portão
  dos tracebacks iguais.
- **O `ty`.** Os limites da seção "Os limites do checker" podem ter
  parentes que as sondas não acharam, e cada um pede uma mudança no fork.

## Geradores, para o fim

O usuário deixou a função com `yield` para o fim do plano do backend, em
13/nov/2026. A fatia 7 não a compila, e a parte 7k dá o diagnóstico. A
expressão geradora que um consumidor recebe na hora, como `sum(x for x
in xs)`, continua como hoje. As duas notas da fatia 6 vão junto: a
tabela `substitutes` sai quando o `<genexpr>` receber o iterador, e
`sum`, `min` e `max` passam a consumir o iterador sem lista. Esta seção
guarda o desenho e o que a análise achou, para quando a parte vier.

A parte compila a função com `yield`, com `yield` como comando e como
valor de uma atribuição, `yield from`, o `return` com valor, a expressão
geradora como valor e a regra da PEP 479, em que um `StopIteration` que
escapa do corpo vira `RuntimeError: generator raised StopIteration`. O
`ty` dá `Unknown` ao valor de `yield from` sobre uma lista ou outro
gerador, e ao `yield` de um gerador anotado com `-> Iterable[int]`, e o
fork precisa disso corrigido antes.

Uma função com `yield` vira uma classe gerada, com o número do estado e o
último item. O corpo usa os locais do wasm, como uma função comum, e a
cada `yield` guarda todos eles no objeto e os lê de volta na retomada.
Isso vale também para as flags de definição, as células e o ambiente da
closure, que hoje são locais do emissor. A lista dos locais que o gerador
guarda sai do emissor, que é quem cria esses locais, e não de uma análise
anterior que tente adivinhá-los. Uma análise assim causou vários crashes
no backend do Gleam. Um local morto também fica guardado, e o objeto que
ele aponta continua vivo até o gerador acabar.

A lowering transforma só os comandos que têm um `yield` dentro. O corpo
vira um laço com um `br_table` sobre o estado, e cada `yield` guarda o
estado seguinte e sai. Um comando sem `yield` dentro continua
estruturado. É a transformação do regenerator do JavaScript, sobre a IR
em árvore. O `ForRange` guarda o contador em temporários do emissor, e
num gerador ele vira um `While` sobre variáveis da IR. O `yield` só é
aceito como comando e como valor de uma atribuição, porque um `yield` no
meio de um operando deixaria valores na pilha do wasm.

Um gerador terminado, ou que lançou, dá `StopIteration()` a cada `next`.
Depois de `return v`, o primeiro `next` dá `StopIteration(v)`, e o
seguinte dá `StopIteration()`. Um gerador que chama `next` sobre si mesmo
lança `ValueError: generator already executing`. O traceback mostra o
quadro do gerador quando ele lança, e cada retomada conta no contador de
profundidade.

A expressão geradora passa a ser um gerador como os outros. A função
`<genexpr>` recebe o iterador do primeiro iterável, como no CPython, e a
tabela `substitutes` sai. Hoje um nome de uma compreensão em volta vai
para o `<genexpr>` como cópia, o que só vale porque o consumidor roda na
hora. Como valor, o gerador captura o nome como uma closure captura.

O gerador salva o global da exceção tratada a cada `yield` e o restaura a
cada retomada. Cada gerador ganha o seu `$Class` e o seu bit na tabela
de `Iterator`, e o `print` de um gerador escreve o endereço sintético
como os outros objetos.

O CPython lança um `GeneratorExit` no `yield` parado quando libera um
gerador abandonado, pela contagem de referências, e o WasmGC não avisa
quando um objeto deixa de ser usado. O código que roda nessa hora fica de
fora, com um diagnóstico:

- o `yield` dentro de um `try` com `finally`;
- o `yield` dentro de um `try` cujo `except` captura o `GeneratorExit`,
  que são o `except:` sem classe, `except BaseException` e `except
  GeneratorExit`;
- o `yield` dentro de um `with`.

Com `for n in g(): break`, um gerador com `try: yield 1` e `except:
print("capturou")` escreve `capturou` no CPython. Um `yield` num `try`
que só tem `except Exception`, ou classes abaixo dela, não tem o
problema. `send`, `throw` e `close` são a outra metade dessa decisão, e o
usuário a toma quando a parte vier.

Uma sondagem mede o tempo de um gerador de 10^6 itens no V8, contra o
CPython e contra a mesma conta numa lista, com o laço numa função chamada
várias vezes. Os casos de borda com o CPython como oráculo:

- um gerador que lança no meio, um gerador terminado, um gerador
  infinito com `zip`, um `StopIteration` dentro de um gerador, um gerador
  que faz `yield` dentro de um `except`, e `yield` dentro de um laço
  dentro de um `if`;
- o desempacotamento de um gerador.

O risco da parte é a máquina de estados. Um estado errado retoma no lugar
errado sem nenhum erro do wasm.

## Decisões

O usuário decidiu os onze pontos em 13/nov/2026, com o critério de
escolher o desenho mais robusto a longo prazo, e não o que basta a um
programa pequeno.

1. A ordem das partes é de 7a a 7k, e os inteiros grandes são a fatia 8,
   logo depois desta.
2. As exceções são classes do prelude em Python, com o descritor
   `$Class`.
3. A iteração usa o `next() -> bool` interno, e o `StopIteration` só
   aparece no `__next__` público, no adaptador do iterador do aluno e no
   `next()` de `map` e de `filter`.
4. A função com `yield` fica para o fim do plano do backend.
5. A herança em que só uma base tem campos entra. O despacho fica atrás
   de uma abstração na IR, com o mecanismo num módulo do emissor: a
   cadeia de `ref.test` para poucos corpos e o id de classe, com
   `br_table` e intervalo, para muitos. A vtable perdeu nas medidas do
   V8 e fica como alternativa.
6. O módulo `abc`, com `ABC` e `abstractmethod`, e o `from __future__
   import annotations` entram, no item 41.
7. A classe como valor geral entra, com `type[C]`, no item 37.
8. O `%` de `str` entra, no item 20.
9. O `with` entra, no item 48, e o `contextlib` vai com os geradores.
10. `NamedTuple`, `TypedDict`, `Flag`, `IntFlag`, `frozenset`, `Sequence`
    e `Mapping` entram, nos itens 18, 23, 24 e 36.
11. O `print` escreve um endereço sintético, tirado do campo de
    identidade, que só existe nas classes que o pedem e é preenchido no
    primeiro uso.

## O que a revisão mudou

Três revisores leram a primeira versão, um técnico, um da semântica do
CPython 3.13 e um do escopo.

- As classes que o runtime lança passaram a ser sempre instanciadas, e o
  traceback passou a usar uma função do prelude para a última linha.
- O quadro da função que captura entra na captura, com a marca que evita
  o quadro dobrado no `raise` sozinho.
- O `leave` do `return` foi para depois do `finally`, e o `try` guarda a
  pilha do `repr`. O local do contador saiu, porque o `reraise` já o
  restaura.
- A cadeia dos tipos estruturais deixou de vir das coerções e passou a
  cobrir todo tipo que satisfaz o protocolo.
- O `cls` e o atributo de classe passaram a despachar pela classe exata,
  e o atributo comum com valor no corpo entrou como default do campo.
- O descritor `$Class` foi para a parte das classes, que o usa antes das
  exceções.
- A tupla virou uma classe gerada.
- O gerador guarda os locais a cada `yield`, em vez de os mover para
  campos, e o `yield` dentro de `finally` e de `with` saiu, porque o
  CPython roda esse código quando libera o gerador.
- O `%` ganhou leitor próprio, e o `str.format` ficou com o modelo
  literal.
- A iteração do prelude veio para antes das exceções, porque o nível 3 a
  usa, e os limites do `ty` vieram para antes dela.
- O `__name__` veio da fase do REPL para a parte dos módulos.
- As exceções ganharam quatro etapas, o formato ganhou parte própria, e
  os inteiros grandes viraram a fatia 8.
- A tabela de níveis foi corrigida, e entraram a variável de dois tipos,
  o `str` de uma união, `class C()`, os métodos especiais numa dataclass,
  `<<` e `>>`, os nomes de `typing` que faltavam, a classe local e as
  opções de `dataclasses` e `enum`.
- A primeira versão dizia que o checker já recusava a maioria do que fica
  de fora. Ele aceita boa parte, e a 7k cobre isso.
- Os casos de borda ganharam as regras do CPython que os revisores
  conferiram no `python3.13`.

## O que a comparação mudou

O jwonder, que compila Java para WasmGC, e o backend wasm do Gleam
resolvem parte destes problemas. A comparação com os dois mudou o plano
assim.

- O `except` passou a ser o `IsInstance`, sem subir pelas bases, e a tag
  continua uma só.
- A lista das classes que o runtime lança passou a sair da tabela do
  emissor.
- O `$Class` virou um global imutável, com o `ref.eq` para `type(x) is
  C`.
- O despacho passou para trás de uma abstração, com o id de classe para
  muitos corpos e a cadeia para poucos. A decisão 5 chegou a propor a
  vtable e a tabela em baldes do jwonder, e a medida do V8 no plano da
  fatia 6 a derrubou.
- A chave da tupla gerada é a lista de `Rep`.
- O corpo de um `case` com `|` sai uma vez.
- A S1 mede só no V8, o laço medido fica fora do corpo do módulo, e a S4
  parte da medida do sgleam.
- Cada divergência declarada ganhou um teste nos dois sentidos.

Ficaram de fora o traceback sem quadros do jwonder, as itables dele, que
servem aos arrays covariantes do Java, e o dict, a string e as chamadas
de cauda do Gleam, que não seguem a semântica do Python.

## O que muda no plano do backend

A lista das fatias passa a ter a fatia 7 como a linguagem completa, com
este plano, e a fatia 8 como os inteiros grandes. As imagens, os
programas interativos, o `input`, o `...` como comando e o capítulo 09
vão para a fatia 10, com o REPL, e o item "As imagens" de "Em aberto"
vai junto. Os geradores viram a fatia 11.
