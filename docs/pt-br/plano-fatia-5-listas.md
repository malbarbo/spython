# Plano: a fatia 5, as listas e a repetição

Este plano detalha a fatia 5 do [plano do backend
WasmGC](plano-backend-wasmgc.md): "Nível 3. Arrays, os métodos especiais,
`list`, `for`, `range`, fatias e `+=`."

A fatia 4, o REPL na CLI, fica para o fim, por decisão do usuário. Esta
fatia vem logo depois da 3.

Cada tipo de lista tem o próprio array wasm, com os itens na forma local
do tipo do item, como o backend do gleam faz. Os métodos de `list`, o
`repr` e o `==` ficam no prelude, em Python, como funções genéricas, e o
compilador gera uma cópia de cada uma para as `Rep` das variáveis de
tipo. O compilador leva `lst.append(x)` à função do prelude pela tabela
de `lower/prelude.rs`. O `for` sobre uma sequência vira um `while` da IR,
e o `for` sobre `range` vira um nó próprio.

## Onde a fatia começa

O compilador cobre os níveis 0 a 2. O teste do corpus verifica os
capítulos 01 a 05 do `na-programacao` e falha quando o compilador recusa
um arquivo desses níveis. A `Rep` tem nove formas, e as classes do
programa são structs num rec group, decididas por `ref.test`. O prelude
não usa `for` nem `list`, e um `StrBuilder` faz o papel de um array de
code points. O compilador aceita `while`, `+=` num nome, os métodos de
`str` que não devolvem lista, `max` e `min` de dois valores, `abs` e
`round`.

O `Shape` do fork já tem `List` e `TypeVar`, e o `ty` dá a uma chamada de
função genérica o tipo especializado. O `rep_of` de hoje não aceita
nenhum dos dois, e `signature` recusa uma função com parâmetros de tipo.

Três lacunas de hoje aparecem nesta fatia:

- Não há `repr` uniforme que devolva um `str`. `add_write_uniform`
  escreve o `str` de um valor, e por isso um `str` num item de união,
  como em `list[int | str]`, sairia sem aspas. `repr_at` também não
  aceita `StrOrNone` nem `None`.
- O `==` uniforme compara por identidade toda referência que não é uma
  caixa, inclusive uma dataclass.
- `method_call` passa todo argumento `bool` para `int`, e
  `xs.append(True)` guardaria `1`.

O nível 3 libera `for`, `+=`, a lista literal e a tupla literal. No
`na-programacao`, os capítulos sem classes depois do 05 são o 06, o 07, o
08, o 09 e o 11, com 55 arquivos. O 10 e o 12 não têm `.py`. O checker
recusa 12 deles no nível 3:

- 5 do capítulo 09 importam `sys` ou usam `open`;
- 4 usam uma comparação encadeada (`insere_pos`, `remove_pos`,
  `remove_indice` e `inverte`);
- 2 deixam a variável do `for` sem uso (`matriz_nula` e `pi`);
- `nova_posicao.py` tem uma variável maiúscula num exemplo de doctest.

Os 43 restantes usam:

- `[]` e listas literais, inclusive aninhadas, como `[[1, 0], [0, 1]]`;
- `len`, `lst[i]`, `lst[i] = v`, `a[i][j]` e `a[i][j] = v`;
- `append` (14 vezes) e `pop()` (1);
- `for x in lst` e `for i in range(...)` com um ou dois argumentos;
- `+` entre listas e a fatia `lst[1:]`, no capítulo 11;
- `==` entre listas, como `lst == vazia`, no capítulo 11;
- `s.split()` sem argumentos (5 vezes), `s.index(',')`, `',' in s`,
  `int`, `str` e `max(a, b)`, que já compilam, menos o `split`;
- `+=` num nome (1);
- `list[int]`, `list[str]`, `list[bool]`, `list[list[int]]` e listas de
  dataclasses;
- o `repr` de listas nos exemplos dos doctests, como `[[4, 7], [5, 8]]`;
- no capítulo 09, `input`, `if __name__ == '__main__'` e `...` como
  comando no corpo de uma função.

Nenhum arquivo dos 55 usa tupla, `sort`, `sorted`, compreensão ou
f-string com formato. Nenhum chama `list.index`, usa `in` numa lista,
`for` sobre um `str`, `break` ou `continue`, nem imprime uma lista fora
de um doctest.

As notas de aula desses capítulos têm transcrições de REPL que vão entrar
no corpus mais tarde. Elas usam um `for` no módulo, `a[2][1] = 0` e, no
capítulo 07, `set()`, que fica para a fatia 6.

## O que a fatia entrega

1. O `repr` uniforme, que devolve um `str`, e o `bool` preservado num
   parâmetro do prelude que não é `int`.
2. A lista, com a literal, `len`, `is`, a verdade, a leitura e a escrita
   de um item, o `print`, o `repr`, o `str` e o `==`.
3. Funções genéricas no prelude, com uma cópia para cada `Rep` das
   variáveis de tipo.
4. O `for` sobre `range`, sobre uma lista e sobre um `str`, com `else`,
   `break` e `continue`, no módulo, numa função e num doctest.
5. `append`, `pop`, `insert`, `extend`, `index`, `count`, `remove`,
   `clear`, `copy`, `reverse`, `in`, `not in`, `+` e as fatias de lista.
6. `split()` sem argumentos e `join` de `str`.
7. Um campo de lista numa dataclass.
8. Os capítulos 06, 07, 08 e 11 no portão do corpus, no nível 3.

Os métodos que nenhum arquivo usa entram porque, com os intrínsecos,
cada um é uma função curta em Python. O campo de lista numa dataclass não
tem uso no corpus, mas é o único jeito de montar uma lista que contém a
si mesma e de testar o `[...]` e a profundidade do `repr` e do `==`.

## O que sai da fatia

| Item | Vai para | Por quê |
|---|---|---|
| métodos especiais e métodos de classe | 6 | a lista do prelude é feita de funções, e só uma classe com métodos, no nível 4, define um |
| a entrada implícita com site (verdade, `==`, campo de f-string) | 6 | vem com os métodos especiais |
| `set` e `set()` | 6 | nível 4, e só aparece nas notas do capítulo 07 |
| `==`, `in`, `index`, `count` e `remove` com itens `Uniform` que podem ser dataclasses, como `list[Ponto \| int]` | 6 | o `==` uniforme de objetos é `ref.eq` até a `$VT` |
| `==` e `+` entre dois tipos de lista, como `list[int]` e `list[bool]` | 8 | a cópia genérica pede o mesmo `T` dos dois lados |
| `list[None]` | 8 | o item não tem forma local |
| tupla, `divmod`, `frexp`, `modf`, `partition` | 8 | nenhum arquivo usa |
| `sort`, `sorted`, `reversed`, `enumerate`, `zip` | 6 e 8 | `key=` precisa de lambda, e o `sort` do CPython é o Timsort |
| `*` de lista, `list()`, `list(xs)`, `list(s)` | 8 | sem uso no corpus |
| `sum`, `min` e `max` de uma lista | 8 | o `sum` de `float` do CPython 3.12 é compensado, e o de `int` só testa os 31 bits no fim |
| `fsum`, `prod`, `sumprod` e `dist` de `math` | 8 | sem uso no corpus, e um builtin em Rust não recebe uma lista |
| `split` com separador ou `maxsplit`, `rsplit`, `splitlines` | 8 | o corpus só usa `split()` |
| `lst[i] += v` e os outros operadores num item | 8 | sem uso no corpus, e o índice é testado de novo na escrita |
| `+=` e `*=` com uma lista no alvo | 8 | estendem a lista no lugar, sem uso no corpus |
| `extend` de um `str` ou de um `range` | 8 | sem uso no corpus |
| `<`, `<=`, `>` e `>=` entre listas | 8 | sem uso no corpus |
| atribuição a uma fatia, `del` | 8 | `del` é do nível 5, e a fatia sem uso |
| `range` como valor, `len(range(...))` | 8 | só o `for` usa `range` no corpus |
| `for c in Cor`, `Cor(1)`, `Cor['X']`, `len(Cor)` | 8 | sem uso no corpus |
| f-string com formato, inclusive de um objeto, e `format` | 7 | o único uso é `{c.segundos:03d}`, no capítulo A0 |
| `print` com `sep` ou `end`, `type`, `isinstance`, `id`, `ascii` | 8 | a fatia 2 os deixou para o primeiro uso, e nenhum arquivo dos capítulos os usa |
| `list[object]` | 8 | o corpus não usa `object` |
| o alvo do `for` que não é um nome | 8 | precisa de tupla ou de um alvo com efeito |
| `input`, `__name__`, `...` como comando e o capítulo 09 no portão | fim, com o REPL | decisão do usuário |

O que fica de fora é `Unsupported`. Isso vale também para um `+=` com
lista no alvo, que hoje cairia no `+` e criaria uma lista nova, quando o
CPython estende a lista que já existe.

## Desenho

### Representação

| Tipo do `ty` | `Rep` | Forma local | Forma uniforme |
|---|---|---|---|
| `list[T]` | `List(L)` | `(ref null $List_L)`, lido com `ref.as_non_null` | `(ref $List_L)` |
| `list[T] \| None` | `ListOrNone(L)` | `(ref null $List_L)` | igual |

`L` é um `ListId`, um índice na tabela de tipos de lista, como o
`ClassId`. A tabela guarda a `Rep` do item de cada tipo, e
`list[list[int]]` é um `ListId` cujo item é `List` de outro `ListId`.
Assim a `Rep` continua `Copy`, e ganha `Hash`. A tabela fica no
`Lowering`, num `RefCell`, e `rep_of` dá a `Shape::List(item)` o
`ListId` da `Rep` do item, criando a entrada na primeira vez. O código do
aluno e as cópias do prelude rodam no mesmo `Lowering` e concordam nos
`ListId`. A chave é a `Rep` do item, e por isso `list[int | str]` e
`list[float | None]`, cujos itens são `Uniform`, dividem o mesmo `ListId`.
O `Program` leva a tabela ao emissor.

Cada `ListId` tem um array e um struct próprios, com o item na forma
local da `Rep` dele:

```wasm
(type $Items_int (array (mut i32)))
(type $List_int (struct (field $len (mut i32)) (field $items (mut (ref $Items_int)))))
(type $Items_float (array (mut f64)))
(type $Items_str (array (mut (ref null $Str))))
(type $Items_Ponto (array (mut (ref null $Ponto))))
;; o item de uma união sem forma própria fica na forma uniforme
(type $Items_uniform (array (mut (ref null eq))))
```

Um `float` fica num `f64`, sem a caixa e sem a alocação por item, que no
coletor DRC do wasmtime custa cerca de 180 ns. Um `int` fica num `i32`,
e a leitura não precisa de `ref.cast` nem de `i31.get_s`. Um item
`str | None` fica como `StrOrNone`, e um `Ponto | None` como
`ObjectOrNone`, que já existem. Só as outras uniões ficam na forma
uniforme. `list[None]` não tem forma local para o item e é
`Unsupported`.

Os tipos de todas as listas entram no rec group das classes, porque uma
dataclass pode ter um campo de lista e uma lista pode ter itens de uma
dataclass. O rec group também faz de `$List_int` e `$List_bool` dois
tipos distintos, apesar de terem o mesmo layout, e assim um `ref.test`
distingue `[True]` de `[1]` numa forma uniforme. O `rec_group` de hoje só
aceita structs, e o `array_type` junta os arrays pelo tipo do item. Os
dois passam a aceitar os arrays de lista no grupo. `Reprs::new` roda
depois do lowering e já tem a tabela inteira.

Uma lista vazia aponta para um array de tamanho 0 global do tipo dela, e
o primeiro `append` troca o array. A capacidade dobra quando acaba, a
partir de 4. O arquivo novo `emit/repr/list.rs` tem o layout das listas,
como `StrRepr` tem o do `str`, e nenhum outro arquivo do emissor sabe dos
campos de um `$List_L`.

`join` dá `ListOrNone(L)` a `List(L)` com `None`, e `holds` aceita os
dois. `widen`, `narrow`, `has`, `is_none`, `global`, `load` (com
`ref.as_non_null`), `truth` e `str_of` ganham os braços de lista, e o
`print` passa a rodar o `write` de uma lista dentro de `at`, porque ele
pode lançar. O commit que traz `rep_of` para as listas recusa o `print`,
o `repr`, o `str`, o `==` e a passagem para a forma uniforme de uma
lista, com um teste de `Unsupported`, até os commits que trazem cada
um.

A lista literal pega o `ListId` do tipo que o `ty` dá a ela, que vem do
contexto em `xs: list[int] = [True]`, e passa cada item à `Rep` do item.
Isso pede o `bool` como `int` (`BoolAsInt`), que o `coerce` de hoje não
faz.

A leitura de um item cujo tipo o `ty` estreitou, como `xs[0]` depois de
`if xs[0] is not None`, passa pelo estreitamento com teste, como o
`narrowed_field` da fatia 3, porque uma chamada entre o teste e a
leitura pode mudar a lista.

Uma lista guardada numa forma uniforme, como numa variável
`list[int] | str`, é a própria referência. O `print`, o `repr`, o `==` e
a verdade de um valor uniforme testam cada `$List_L` do programa por
`ref.test`, como já testam cada classe. O plano do backend manda esse
despacho pela `$VT`. Esta fatia ainda não tem `$VT`, e a troca fica para
a fatia 6, como na fatia 3.

### Dois tipos de lista numa operação

`[1] == [True]` é `True` no CPython, mas `list[int]` e `list[bool]` têm
`ListId` distintos. O `==` de lista do prelude recebe `list[T]` dos dois
lados, e não há um `T` só para os dois. O mesmo vale para `list[int]`
contra `list[int | None]`, para o `+` de dois tipos de lista, que o `ty`
tipa como `list[_S | _T]`, e para um `==` uniforme entre dois `$List_L`
diferentes, que cairia na identidade. Esses casos são `Unsupported`, com
o teste descendo pelos tipos dos itens. O teste olha os tipos estáticos
dos dois lados, inclusive de um `==` uniforme cujos lados podem guardar
listas de tipos diferentes.

### O `repr` uniforme

Com a monomorfização, o `repr` de uma lista chama o `repr` do item
direto, pela `Rep` dele. Só o item `Uniform` e uma lista guardada numa
forma uniforme precisam do `repr` decidido em tempo de execução. Um nó
novo, `Expr::UniformRepr`, dá o `repr` de um valor uniforme como `str`.
O emissor testa as caixas, que ele já sabe escrever, e para o resto
chama uma função que a IR lhe dá:

- `Program.str_repr`, o `repr_of` de `str.py`;
- `UserClass.repr`, a função gerada de cada classe que pode chegar a um
  `repr`;
- `ListType.repr`, a cópia do `repr` de lista de cada `ListId`.

O emissor não monta nenhum texto. Os `ListId` aparecem durante o lowering,
no código do aluno, nos campos das dataclasses e dentro das cópias do
prelude, e o `coerce` é uma função livre que não registra nada. Por isso
o lowering não sabe de antemão quais listas chegam a uma forma uniforme.
Depois do laço que compila as funções, em `lower()`, um ponto fixo dá a
cada `ListId` a cópia do `repr`, e do `==` quando o programa compara
valores uniformes, sempre que o programa escreve, mostra ou compara um
valor uniforme. Uma cópia nova pode criar um `ListId` novo, e o ponto
fixo para quando nada novo aparece. O `write` uniforme também usa
`ListType.repr`, porque o `str` de uma lista é o `repr`.

`repr_at` passa a aceitar `Rep::Uniform`, pelo nó novo, e também
`StrOrNone` e `None`. O `IfElse` que hoje só `field_repr` usa para
`StrOrNone` muda para `repr_at`, e a cópia do `repr` de lista com
`T = StrOrNone` funciona. O `repr` de um campo de uma união, que a fatia
3 deixou de fora, passa a valer.

O `print` de uma lista monta o `str` antes de escrever, como o CPython,
que só escreve um argumento depois de o `str()` dele dar certo. Um
`RecursionError` no meio do `repr` de uma lista deixa escritos os
argumentos anteriores e os separadores, e nada da lista.

`classes_in` passa a descer em `Shape::List`, e assim `print(pontos)`
gera o `repr` de `Ponto`, e `print([Cor.X])` o de `Cor`, que é
`<Cor.X: 1>`, diferente do `str`.

### O `repr` e o `==` de lista, no prelude

O `repr` de uma lista é uma função genérica do prelude. Ela chama
`repr(x)` em cada item, junta as partes com `join` e põe os colchetes.
Por isso o `join` de `str` vem no mesmo commit. Até o `for` existir, o
prelude anda pela lista com `while` e o intrínseco `item`. A cópia com
`T = Int` chama o `repr` de `int`, e a cópia com `T = List(L)` chama a
cópia com `T` igual à `Rep` do item de `L`.

Uma lista que aparece dentro do próprio `repr` escreve `[...]`, pela
pilha de objetos em curso que o `repr` de uma dataclass usa (`in_repr`).
Cada tipo escreve a própria marca. Uma dataclass dentro do próprio
`repr` escreve `...`, e uma lista escreve `[...]`. A pilha passa a
existir sempre que o programa tem um `repr` de lista.

O `==` entre listas é falso quando os tamanhos diferem, e depois compara
os itens em ordem, com o item da esquerda à esquerda. O `==` dos itens
sai da `Rep` de `T` em cada cópia, e a cópia com `T = List(L)` chama a
cópia do item de `L`. Assim `[[1]] == [[1]]` desce nas listas de dentro.
Na cópia com `T = Object(C)` ou `ObjectOrNone(C)`, o `==` é a função
gerada da dataclass, que a fatia 3 já tem, e `Ponto(1, 2) in pontos`
compara os campos, como no CPython. O `==` de lista e o de dataclass
começam pela identidade. Só um item `Uniform` passa pelo `==` uniforme,
que ganha um braço para cada `$List_L` e chama `ListType.eq`. O `in`, o
`index`, o `count` e o `remove` usam o mesmo `==`.

O `==` uniforme de dois objetos ainda é `ref.eq`. Por isso o `==`, o
`in`, o `index`, o `count` e o `remove` de uma lista cujo item é
`Uniform` e pode guardar uma dataclass, como `list[Ponto | int]`, são
`Unsupported`. O teste fica na chamada do aluno e desce nos itens das
listas aninhadas, porque dentro da cópia o tipo é só `T`.

No CPython, o `repr` e o `==` de lista são código C. Eles não aparecem no
traceback, e a profundidade deles conta contra o limite de recursão em C,
não contra o de 1000 quadros. O `==` de listas aninhadas falha na
profundidade 9999, com `RecursionError: maximum recursion depth exceeded
in comparison`, e o `repr` na 9998, com `... while getting the repr of an
object`. As funções do prelude não têm quadro nem a conta de recursão.
Um contador global novo, com dois intrínsecos para entrar e sair, conta
essa profundidade, e o prelude lança o `RecursionError` com a mensagem
do CPython. Uma lista que contém a si mesma é igual a si mesma pela
identidade, e dois ciclos distintos chegam ao limite. Um `repr` de
dataclass dentro de um `repr` de lista mostra só os quadros da dataclass,
como no CPython.

As cópias do prelude passam a chamar o `repr` e o `==` gerados das
dataclasses enquanto o arquivo do lowering é o prelude. Isso funciona,
porque essas funções registram os próprios sites e `site()` dá
`Site::NONE` no prelude, mas é a primeira vez que acontece.

### O que fica à mão e o que vai para o prelude

Fica à mão, em `emit/repr/list.rs`, o que precisa de uma instrução de
array ou roda em todo laço:

- a lista literal, com um `array.new_fixed` de até 64 itens e
  `array.new` mais `array.set` acima disso, pelo limite de 10 mil do V8;
- `len`, `is`, `lst[i]` e `lst[i] = v`, com o índice negativo e o
  `IndexError` (`list index out of range` e `list assignment index out
  of range`). Numa atribuição, o CPython avalia o valor, depois a lista e
  depois o índice.

Vai para o prelude o resto. Os intrínsecos novos em `intrinsics.pyi`
são genéricos, e cada um vira um nó da IR com o `ListId`:
`list_with_capacity(n)`, `list_capacity(xs)`, `list_set_len(xs, n)`,
`item(xs, i)` e `set_item(xs, i, x)` sem teste de índice,
`copy_items(dst, at, src, start, count)`, que é um `array.copy`, e a
entrada e a saída do contador de profundidade. O `list_with_capacity`
pega o `ListId` do tipo que o `ty` dá à chamada, que vem do local anotado
ou do retorno, e o commit dele confere que o `ty` especializa o `T` nos
dois casos. Com eles, `append`, `pop`, `insert`, `extend`, `index`,
`count`, `remove`, `clear`, `copy`, `reverse`, `in`, `+`, o `repr`, o
`==` e as fatias ficam em Python.

Os detalhes do CPython que os testes conferem:

- `[].pop()` e `[].pop(5)` dão `pop from empty list`, porque a lista
  vazia é testada antes do índice. `[1].pop(5)` dá `pop index out of
  range`.
- `remove` dá `ValueError: list.remove(x): x not in list`, e `index` dá
  `ValueError: 9 is not in list`, com o `repr` do valor.
- `index(x, start, stop)` e `insert(i, x)` ajustam o índice como uma
  fatia, sem erro.
- `extend` lê o tamanho do argumento antes de crescer, porque
  `xs.extend(xs)` dobra a lista e para.
- Uma fatia com passo 0 dá `ValueError: slice step cannot be zero`.

As fatias de lista usam `slice_start`, `slice_end` e `step_bound`, que
hoje estão em `str.py`, e eles mudam para um módulo comum do prelude,
listado em `FILES` de `compiler/prelude/src/lib.rs`. `slice_step` monta
um `str` e fica onde está.

A "Em aberto" do plano do backend registra que a atribuição a um
subscrito no prelude vira `StrSet` sem olhar o alvo. Com as listas, a
instrução sai da `Rep` do alvo, e o `StrBuilder` entra no mesmo `match`.

### Funções genéricas no prelude

As funções do prelude que recebem uma lista são genéricas, com a sintaxe
do PEP 695:

```python
def append[T](xs: list[T], x: T) -> None:
    n = len(xs)
    if n == list_capacity(xs):
        grow(xs, n + 1)
    set_item(xs, n, x)
    list_set_len(xs, n + 1)
```

O compilador gera uma cópia de cada função genérica para cada
combinação de `Rep` das variáveis de tipo, como o `function_generic` do
backend do gleam (`compiler-core/src/webassembly.rs` e
`monomorphize.rs`, no `gleam-sgleam-wasm`). O gleam casa tipos com
tipos. Aqui o casamento é com a `Rep` dos argumentos já compilados,
porque dentro de uma função genérica o tipo de um argumento ainda é o
`T` de quem chama, e porque a chamada de um método, como
`xs.append(x)`, chega pelo stub `list.append(self, object: _T)` e não
pela função do prelude.

- **O casamento.** `call` e `method_call` compilam os argumentos
  primeiro. Depois casam o `Shape` declarado de cada parâmetro, que vem
  do `function_info` da função do prelude, com a `Rep` do argumento.
  `List(TypeVar T)` contra `Rep::List(L)` dá `T` igual à `Rep` do item
  de `L`. Só uma posição dentro de `list[...]` liga o `T`, porque a
  lista é invariante. Uma posição `T` solta só converte o argumento, com
  `BoolAsInt` quando `T` é `Int`, e assim `xs.append(True)` com
  `xs: list[int]` guarda `1`. Um `T` que só aparece solto, ou duas
  ligações diferentes do mesmo `T`, são `Unsupported`. Um `T` que só
  aparece no resultado também.
- **O mapa fica na assinatura.** O resultado do casamento é um mapa do
  nome de cada variável de tipo para uma `Rep`, guardado no `Sig`, na
  ordem de `def.type_params`. `signature(def, mapa)` o recebe, porque
  hoje `signature` roda no escopo de quem chama, e `func_of` o copia para
  o `Scope` da cópia. `rep_of(Shape::TypeVar)` o consulta no escopo. O
  `ty` dá só o nome da variável, e o nome basta dentro de um corpo.
- **A cópia.** O corpo é o mesmo, e cada cópia é uma função própria.
  `prelude_funcs` passa a guardar a função pelo arquivo, pelo nome e
  pelas `Rep` do mapa. Uma função genérica não entra em `funcs.ids`,
  que vai do `DefId` a uma função só, e toda chamada a ela passa por
  `prelude_def`. A cópia é compilada na primeira chamada, como uma
  função do prelude hoje, e uma cópia que chama outra, como o `repr` de
  `list[list[int]]`, desce na pilha do Rust.
- **A chave é a `Rep`, não o `Shape`.** `list[Ponto]` e `list[Tempo]`
  têm `Rep` distintas e duas cópias, porque o array é de outro tipo. As
  uniões cuja `Rep` é `Uniform` dividem a mesma cópia, e
  `str | None` e `Ponto | None` têm cópias próprias.
- **O resultado.** A cópia devolve a `Rep` de `T` no mapa, que é a que o
  `ty` dá à chamada especializada.

`signature` aceita parâmetros de tipo numa função do prelude. O
`method_call` de hoje passa todo argumento `bool` para `int`, e passa a
fazer isso só quando o parâmetro é `Int`. Numa cópia com `T = Bool`, o
`bool` fica `bool`, e `print([True])` escreve `[True]`.

Num `T` sem limite, o `ty` só aceita o `==`. Por isso o `sum`, o `min` e
o `max` de lista, que precisam de `+` e de `<`, ficam para depois.

O aluno não escreve funções genéricas no nível 3, e o `TypeVar` fora do
prelude continua `Unsupported`. Quando o aluno escrever, no nível 5, a
recursão polimórfica pode gerar tipos sem fim, e o compilador vai
precisar de um limite de profundidade, como o `TYPE_DEPTH_LIMIT` do
gleam.

### O `for`

`for x in seq`, com `seq` uma lista ou um `str`, vira nós que a IR já
tem, como pede o cabeçalho do `lib.rs`. O lowering guarda `seq` e um
índice em variáveis sem nome (`add_unnamed`) e escreve um `while`. O
teste do `while` compara o índice com o tamanho atual. O começo do corpo
lê o item na variável do laço e soma 1 ao índice, e por isso o
`continue` do `while` de hoje funciona. `seq` é avaliado uma vez, e
`xs = xs + [5]` no corpo não muda o laço. Um `append` dentro do laço
estende o laço, e um `pop` o encurta, como o iterador de lista do
CPython.

`for i in range(...)` vira um nó novo, `Stmt::ForRange { var, start,
stop, step, body, orelse }`. O `while` não serve, porque a soma do passo
depois do último valor pode sair dos 31 bits e lançaria o erro de limite.
O contador soma em `i32` sem o teste, porque `start` e `step` cabem em
31 bits e a soma cabe em 32. `start`, `stop` e `step` são avaliados uma
vez, na ordem do texto. Um `step` 0 lança `ValueError: range() arg 3
must not be zero`, com as marcas sob a chamada de `range`. O contador é
separado da variável do laço, e `i += 10` no corpo não muda o laço. O
`continue` pula para a soma do contador.

A variável do laço é uma atribuição comum, no módulo, numa função e num
doctest. `Bindings`, em `lower/flow.rs`, ganha o braço do `for`, conta o
alvo em `assigned` e em `order`, e `breaks` olha também o `else` de um
`for`. A `Rep` da variável vem do tipo do item: o item de `List(item)`,
`str` para um `str` e `int` para `range`. Um laço vazio deixa a variável
sem valor, e a leitura lança `NameError` no módulo e `UnboundLocalError`
numa função, pela flag de hoje. O `else` roda quando o laço acaba sem
`break`.

O alvo que não é um nome e o `for` sobre outro iterável, como um `range`
guardado numa variável, são `Unsupported`.

### `str` com listas

`split()` sem argumentos e `join` ficam no prelude, em Python. O `split`
separa pelo `is_space` de `str.py`, que é o `str.isspace` do CPython. O
`join` vem com o `repr` de lista, que o usa.

### Dataclass com lista

Um campo `list[T]` entra na forma aceita da fatia 3. O `repr` gerado
escreve o campo pelo `repr` de lista, e o `==` gerado compara pelo `==`
de lista. Um default `[]`
lança no `class` o `ValueError: mutable default <class 'list'> for field
xs is not allowed: use default_factory` do CPython, com os quadros de
`dataclasses.py` que a fatia 3 já dá.

## O que vai para o fim, com o REPL

O usuário deixou o `input` para o fim, junto com o REPL. Todo arquivo
do capítulo 09 que o checker aceita no nível 3 chama `input`, e só
esses arquivos usam `__name__` e `...` como comando. Os três vão juntos,
e o capítulo 09 entra no portão com eles. O desenho fica registrado
aqui.

### `__name__` e `...`

`__name__` vale `'__main__'` num programa e `'__doctest__'` nos doctests,
como no runner, que roda o arquivo num módulo com esse nome. Assim o
`main()` do capítulo 09 não roda durante os doctests.

`...` como comando não faz nada, numa função como já acontece numa
classe.

### `input`

`input()` e `input(prompt)` escrevem o prompt, esvaziam o buffer do
stdout e leem uma linha do stdin pelo `fd_read` do WASI. O prompt vai
para o stdout, menos quando o stdin e o stdout são os dois um terminal,
caso em que o CPython o escreve no stderr. O módulo descobre isso pelo
`fd_fdstat_get`. A linha volta sem o `\n` final. No Linux, o CPython não
traduz `\r\n`, e o `\r` fica na linha. Uma última linha sem `\n` volta
como está. O fim do stdin lança `EOFError: EOF when reading a line`.

O CPython decodifica o UTF-8 por bloco lido, e não por linha. Uma
sequência inválida lança `UnicodeDecodeError` na chamada que lê o bloco,
mesmo quando a linha dela é válida, com a posição contada desde os bytes
pendentes do bloco anterior, e com um quadro `File "<frozen codecs>",
line 325, in decode` sem linha de código. O módulo faz o mesmo. Ele lê
blocos de até 8192 bytes, guarda os bytes depois do `\n` entre as
chamadas e decodifica de forma incremental, com a regra do maior
subprefixo válido do CPython. Um bloco que termina no meio de uma
sequência a completa no bloco seguinte, e o fim do stdin com bytes
pendentes dá `unexpected end of data`.

Um intrínseco devolve os code points da linha, ou um estado de fim ou de
erro com a posição e o tipo do erro. O prelude monta a exceção, e as
mensagens vêm do `cpython/`, como manda o `lib.rs`.

Os testes dão ao programa e ao CPython um stdin explícito, vazio ou com
o texto do teste, porque o processo filho herda o stdin por padrão. O
CPython roda com `PYTHONIOENCODING=utf-8:strict`, sem `-I`, porque num
locale C ele usaria `surrogateescape` e não lançaria. O
`compiler/tests/wasi.ts` ganha o `fd_read` e o `fd_fdstat_get`. O teste
do corpus roda com o stdin vazio, e os arquivos do capítulo 09 mostram o
prompt e o `EOFError`, iguais aos do CPython.

## Etapas

Cada linha é um commit, e cada commit passa pelo portão rápido.

**A. Preparação**

1. `compiler: keep a bool argument of a prelude parameter not int`
2. `compiler: give the repr of a uniform value as a str`

**B. A lista**

3. `compiler: compile a list literal, len, is and the truth of a list`,
   com `Rep::List`, `Rep::ListOrNone`, a tabela de tipos de lista, os
   arrays no rec group e o `rep_of`. O `print`, o `repr`, o `==` e a
   forma uniforme de uma lista são `Unsupported` até os commits 7 e 13
4. `compiler: copy a generic prelude function for the reps of its types`,
   com o mapa no `Sig` e no `Scope`, o casamento pela `Rep`, o cache e o
   desvio de `funcs.ids`, e um teste que conta as funções, como o do fim
   de `lower/prelude.rs`
5. `compiler: add the intrinsics that grow a list`
6. `compiler: compile append of a list`, com o roteamento de
   `method_call` e a regra do `bool`
7. `compiler: write a list in print, repr and str`, com o `join`, o
   intrínseco `item`, o contador de profundidade e o ponto fixo de
   `ListType.repr`
8. `compiler: read an item of a list with IndexError`
9. `compiler: choose the instruction of an item assignment by the rep`
10. `compiler: assign an item of a list`

**C. O `for`**

11. `compiler: compile for over a list and a str`
12. `compiler: compile for over a range`

**D. Os métodos**

13. `compiler: compile == and != of lists`, com `ListType.eq` e o
    `Unsupported` de dois tipos de lista
14. `compiler: compile pop, insert and extend of a list`
15. `compiler: compile index, count, remove and in of a list`
16. `compiler: compile clear, copy and reverse of a list`
17. `compiler: compile + of lists`
18. `compiler: move the bounds of a slice to a shared prelude module`
19. `compiler: compile a slice of a list`
20. `compiler: compile split of str`

**E. Dataclass**

21. `compiler: accept a field of a list in a dataclass`

**F. Fecho**

22. `compiler: check chapters 06 to 08 and 11 of the corpus at level 3`,
    com os quatro capítulos em `CHAPTERS` no `Level::Repetition` e
    `COVERED` em `Level::Repetition`
23. `docs: record the slice 5 in the plan of the backend`, com as medidas
    da seção seguinte

## Testes

- Cada commit traz os testes do que ele compila, em
  `compiler/tests/run/`, com o CPython como oráculo do stdout, do stderr
  e do código de saída, no wasmtime e no deno.
- Cada forma recusada tem um teste de `Unsupported` com a mensagem,
  inclusive o `+=` com lista no alvo, o `in` numa lista de
  `Ponto | int`, `[1] == [True]` e `list[None]`.
- Casos de borda com o CPython como oráculo:
  - o índice negativo, o índice fora, as duas mensagens de `IndexError`
    e as marcas de `a[i][j]`;
  - o `append` e o `pop` dentro de um `for` sobre a mesma lista, e a
    lista trocada no corpo;
  - o `range` com passo negativo, com passo 0 e perto de ±2^30, e a
    variável mudada no corpo;
  - a variável do `for` depois de um laço vazio, no módulo e numa função;
  - o `repr` de uma lista que contém a si mesma por um campo de
    dataclass, o `==` de dois ciclos e as listas aninhadas até 9998 e
    9999;
  - o `print` com uma lista cujo `repr` lança;
  - `xs.extend(xs)`, `pop` de lista vazia com índice e os ajustes de
    `index` e `insert`;
  - o default de função `[]`, que é a mesma lista em toda chamada;
  - `print([True])`, `print(['a'])`, `print([Cor.X])`, `print([None,
    'a'])` e `print([1, 'a'])`;
  - `xs: list[int] = [True]` e `xs.append(True)` com `xs: list[int]`;
  - `Ponto(1, 2) in pontos` e `==` de `list[Ponto | None]`;
  - o `split()` com os espaços do Unicode;
- O relatório dos doctests com listas é igual ao do runner.
- O portão do corpus sobe para o nível 3 no commit 22.

## Medidas

O commit 23 mede, no wasmtime e no deno, contra o CPython:

- a soma de uma `list[int]` de 10^7 itens com `for`;
- o mesmo com `list[float]`;
- 10^7 `append` numa lista vazia.

O commit mede também o tamanho do módulo de um programa que usa listas
de quatro tipos de item, para ver o custo das cópias.

## Riscos

- **O `repr` uniforme.** Ele liga o emissor a funções do lowering pela
  IR, e é a primeira vez que o emissor chama uma função do prelude. Um
  erro na reserva dessas funções aparece em todo `print` de lista.
- **A memória que acaba.** O `trap.rs` só troca por `MemoryError` um
  trap não capturado no wasmtime da CLI. Um doctest não o captura, e o
  deno não o recebe. A "Em aberto" do plano do backend já registra isso.
- **O tamanho do módulo.** Cada tipo de item gera uma cópia de cada
  método usado, além do array e do struct. As funções são curtas e só as
  alcançadas entram, mas o download no navegador cresce com o número de
  tipos de lista do programa.
- **O casamento.** O casamento do `Shape` do parâmetro com a `Rep` do
  argumento é código novo no lowering, e muda a ordem de `call` e de
  `method_call`, que passam a compilar os argumentos antes de escolher a
  função. Um caso que ele não cobre tem de virar `Unsupported`, e não
  uma cópia errada.
- **O ponto fixo.** As cópias de `ListType.repr` e `ListType.eq` saem
  depois do laço das funções e podem criar tipos de lista novos. Um erro
  ali aparece só nos programas que guardam listas em forma uniforme.
- **O `[]` no prelude.** O lint `dynamic-type` (decisão 15 do plano do
  backend) recusa `list[Unknown]` no código do aluno. No prelude, que não
  passa pelos lints, um local sem anotação pode cair nesse tipo.

## Em aberto

- A especialização pelo código de um argumento função, como o
  `specialization.rs` do gleam, quando o `sort` com `key=` e as lambdas
  entrarem.
- O `sort`, com o Timsort do CPython, para a ordem igual com `nan` e com
  `key=`.
- O `sum` de lista, com a soma compensada de `float` do CPython 3.12 e o
  teste dos 31 bits só no resultado.

## O que a revisão mudou

- Os itens deixaram a forma uniforme. Cada tipo de lista tem o próprio
  array, e as funções genéricas do prelude são monomorfizadas pela `Rep`,
  como no backend do gleam (decisão 1).
- O `repr` uniforme entrou, porque o `print` de um item de união `str`
  sairia sem aspas.
- O `==` de lista veio para antes do `in` e do `index`.
- A segunda revisão, depois da monomorfização, pôs o mapa das variáveis
  de tipo no `Sig`, casou os parâmetros com a `Rep` dos argumentos e não
  com o `Shape`, tirou as funções genéricas de `funcs.ids`, recusou as
  operações entre dois tipos de lista, deu ao `repr` de lista um ponto
  fixo depois do laço das funções, trouxe o `join` para o commit do
  `repr` e partiu o commit da monomorfização em três.
- A profundidade do `repr` e do `==` de lista segue o limite em C do
  CPython, com as mensagens dele.
- O `print` de uma lista monta o texto antes de escrever.
- O `bool` num parâmetro do prelude que não é `int` não vira mais `int`.
- O `for` sobre uma sequência virou um `while` da IR, e só o `range`
  ganhou um nó.
- `__name__` e `...` entraram, porque o capítulo 09 precisa deles, e
  depois foram para o fim com o `input` (decisão 5).
- O `input` não traduz `\r\n` no Linux, decodifica por bloco como o
  CPython e escreve o prompt no stderr num terminal.
- `lst[i] += v`, `+=` de lista, `*`, `list()`, `sum`, `min`, `max`, os
  quatro de `math`, o `split` com argumentos, `rsplit` e `splitlines`
  saíram, sem uso no corpus.
- O `in`, o `index`, o `count` e o `remove` com itens `Uniform` que
  podem ser dataclasses são `Unsupported`, como o `==`.
- As contagens passaram a ser dos 43 arquivos aceitos.

## Decisões

O usuário decide estes pontos, uma pergunta por vez.

1. Em 11/nov/2026 o usuário pediu para olhar como o gleam monomorfiza e
   decidiu pela mesma estratégia. Cada tipo de lista tem o próprio array,
   e as funções genéricas do prelude ganham uma cópia por `Rep` das
   variáveis de tipo.
2. Os métodos especiais, com a entrada implícita com site, saem desta
   fatia e vão para a 6. O usuário concordou em 11/nov/2026.
3. A tupla sai desta fatia e vai para a 8, com `divmod`, `frexp`,
   `modf`, `partition` e o alvo em tupla do `for`, apesar de o nível 3
   liberar a tupla literal. Nenhum arquivo dos capítulos a usa. O
   usuário concordou em 11/nov/2026.
4. Um `float` dentro de uma lista perde a identidade. Com `x` um `nan`,
   `[x] == [x]` e `x in [x]` dão `False`, diferente do CPython. Com
   `y = xs[0]` um `nan`, `xs.index(y)` e `xs.remove(y)` lançam
   `ValueError`, onde o CPython acha o item. O usuário aceitou a
   divergência em 11/nov/2026. O CPython guarda todo `float` numa caixa,
   e o compilador guarda o `f64` sem caixa para não alocar a cada conta.
5. O `input` fica para o fim, junto com o REPL, por decisão do usuário
   em 11/nov/2026. O `__name__`, o `...` como comando e o capítulo 09 no
   portão vão com ele, porque só os arquivos que chamam `input` os usam.
   O capítulo 11 entra no portão no nível 3.
