# Plano: a fatia 6, as classes e o nível 4

Este plano detalha a fatia 6 do [plano do backend
WasmGC](plano-backend-wasmgc.md): "Nível 4. Classes com herança, `dict`,
`set`, compreensões, lambda e closures." A fatia 5 mandou para cá também
os métodos especiais, a entrada implícita com site e o `==` de itens
uniformes que podem ser dataclasses.

A fatia é a maior do plano e se divide em três partes. A parte 6a traz
as classes com métodos, a herança e os métodos especiais. A parte 6b
traz `dict` e `set`, que precisam do `__eq__` e do `__hash__` das
classes. A parte 6c traz as funções como valores, as compreensões e o
`sort`, que precisa de `key=`. O portão do corpus sobe para o nível 4 só
no fim da 6c, porque o nível 4 libera as três partes juntas.

O compilador vê o programa inteiro. Uma chamada de método vai direto
para o corpo quando só um corpo é possível. Nas outras, ela testa a
classe exata do objeto numa cadeia de `ref.test` contra tipos `final` e
chama o corpo certo direto. As classes não ganham vtable. A seção "O que
vem do jwonder" mostra as medidas que decidiram isso.

## Onde a fatia começa

O compilador cobre os níveis 0 a 3. Uma dataclass e um Enum são structs
num único rec group, todos `final` e sem supertipo (`emit/module.rs`
declara `is_final: true` para todo tipo), sem cabeçalho e sem vtable.
`print`, `repr` e `==` de um valor uniforme passam por
`add_uniform_dispatch`, que testa os tipos builtin e depois cada classe
com `ref.test`. As listas são monomorfizadas pela `Rep` dos itens, e as
funções genéricas do prelude ganham uma cópia por `Rep` das variáveis de
tipo.

O código de hoje supõe que a classe estática de um valor é a classe
exata dele. `dataclass_eq` chama o `__eq__` da classe estática,
`classes_in` e `uniform_dataclasses` devolvem só a classe nomeada, o
`print` de um `Rep::Object(C)` chama a função de `C`, e `widen` e
`narrow` não têm o caso de uma classe para a base dela. A herança quebra
essas quatro suposições.

Os valores que o `_start` monta, como os literais de `str`, os membros de
Enum e as classes das exceções, ficam todos numa função só. O jwonder
mediu que o Cranelift leva um tempo que cresce como n^2,8 com o número de
alocações numa função, 26,5 s para 800 alocações. A fatia acrescenta mais
valores desses, e por isso começa partindo essa função.

O fork do `ty` já responde o que a fatia pergunta. `class_info` dá a MRO,
os métodos com a classe que cada um sobrescreve (`Method::overrides`), os
atributos que os métodos atribuem em `self` (`instance_attributes`) e os
campos de uma dataclass. `call_target` dá `CallTarget::Method` com a
classe onde a busca achou o método, e `attribute_owner` dá a classe dona
de um atributo.

O nível 4 libera, além do que o nome diz:

- métodos numa classe, inclusive numa dataclass e num Enum;
- `dict` e `set` literais, as compreensões e as expressões geradoras;
- `lambda`;
- o valor default de um parâmetro, que o compilador já aceita numa
  função e vai aceitar num método;
- a verdade de qualquer valor no `if`, no `while`, no `assert`, na
  expressão condicional, no filtro de uma compreensão e nos operandos de
  `and`, `or` e `not`;
- `bool` numa conta, como `True + 1`, que o compilador já aceita;
- a comparação encadeada, que o compilador já aceita entre números e
  `str`, mas não entre objetos;
- a expressão como comando, que o compilador já aceita;
- a segunda definição de uma função ou de uma classe com o mesmo nome.

O corpus quase não tem programas do nível 4. Os capítulos 10 e 12 do
`na-programacao` não têm `.py`, e o 09 fica para o fim com o `input`. Em
`cli/tests/inputs`, só `ok_class.py` é uma classe com métodos, e o teste
do corpus dá a esses arquivos o nível 5. Por isso o portão desta fatia
são os testes do compilador, com o CPython como oráculo. O capítulo A0 e
a lib `spython` da fatia 7 vão usar funções como valores, como o
`to_image` de `World`.

## O que a fatia entrega

**6a. Classes**

1. Uma classe com campos anotados, `__init__` e métodos, e métodos numa
   dataclass e num Enum, com defaults e argumentos nomeados.
2. A herança simples entre classes do programa, com `super()`.
3. A chamada de método pelo programa inteiro, direta quando só um corpo é
   possível e por uma cadeia de `ref.test` nos outros casos.
4. `object` como tipo de parâmetro, e `isinstance` com uma classe do
   programa e com os tipos builtin.
5. Os métodos especiais `__str__`, `__repr__`, `__eq__`, `__ne__`,
   `__lt__`, `__le__`, `__gt__`, `__ge__`, os operadores aritméticos com
   os refletidos e os de `+=`, `__len__`, `__bool__`, `__contains__`,
   `__getitem__` e `__setitem__`.
6. A verdade de qualquer valor, e `and` e `or` com operandos de tipos
   diferentes.

**6b. `dict` e `set`**

7. O `hash` do CPython para `int`, `float`, `bool`, `None`, `str`, um
   membro de Enum, uma dataclass `frozen` e uma classe com `__hash__`.
8. `dict`, com a literal, `d[k]`, `d[k] = v`, `in`, `len`, `get`, `pop`,
   `clear`, `copy`, o `for` sobre o `dict`, as chaves e os valores, o
   `for k, v in d.items()`, o `print` e o `==`.
9. `set`, com a literal, `set()`, `set(xs)`, `add`, `remove`, `discard`,
   `in`, `len`, `copy`, `|`, `&`, `-`, `^`, as comparações, o `for` e o
   `print`, na ordem do CPython.

**6c. Funções como valores**

10. O nome de uma função como valor, a chamada de um valor de função, o
    `lambda` e o `def` dentro de uma função, com as variáveis capturadas.
11. As compreensões de lista, de `set` e de `dict`, e a expressão
    geradora dentro de `sum`, `any`, `all`, `min`, `max`, `sorted` e
    `join`.
12. `sum`, `min`, `max`, `any` e `all` de uma lista, `sort` e `sorted`,
    com `key=` e `reverse=`.
13. O portão do corpus no nível 4.

A fatia 5 mandou `sum`, `min` e `max` de uma lista para a fatia 8. Eles
voltam para esta, porque a expressão geradora usa o mesmo código.

## O que sai da fatia

| Item | Vai para | Por quê |
|---|---|---|
| atributo de classe com valor, como `total: int = 0` numa classe comum | 8 | é compartilhado pela classe e lido pela instância, e nenhum programa usa |
| `@staticmethod`, `@classmethod` e `@property` | 8 | sem uso no corpus, e a fatia 5 já mandou os métodos de classe para cá sem uso |
| herança múltipla, `abc` e classe abstrata | 8 | a MRO de várias bases muda a busca e a cadeia de testes |
| uma base que não é classe do programa, como `Exception` ou `IntEnum` | 8 | `try` é do nível 5, e `IntEnum` só aparece em `cli/tests/inputs` |
| um campo que a subclasse redeclara com outro tipo | 8 | um campo mutável de um struct não muda de tipo num subtipo |
| `NotImplemented` como resultado de um método especial | 8 | pede a segunda busca em tempo de execução |
| `__iter__`, `__next__`, `__call__` | 8 | o `for` sobre um objeto pede `StopIteration`, que pede `try` |
| `hash`, `id`, `set` e chave de `dict` de uma classe sem `__eq__` | 8 | o `hash` vem do endereço no CPython, e a classe precisaria de um campo para ele |
| `~` de um `bool` | 8 | o CPython 3.13 escreve um `DeprecationWarning` |
| `d.items()`, `d.keys()` e `d.values()` fora de um `for`, `dict()`, `update`, `setdefault` | 8 | `items()` dá tuplas, que estão na fatia 8 |
| `map`, `filter`, `enumerate`, `zip`, `reversed` | 8 | dão iteradores preguiçosos e, com exceção de `map` e `filter`, tuplas |
| expressão geradora fora dos consumidores da lista acima, e `yield` | 8 | um gerador como valor precisa guardar o estado entre os passos |
| um builtin, um método de `str` ou de lista e um método ligado como valor, como `key=len` e `obj.m` | 8 | pedem uma função adaptadora por uso |
| um valor de função cujos parâmetros têm `Rep` diferente da esperada, como `def f(x: object)` onde se espera `Callable[[int], int]` | 8 | pede uma função adaptadora |
| `nonlocal` e `global` numa closure | 8 | `nonlocal` é do nível 5 |

O que fica de fora é `Unsupported`.

## Desenho

### A forma aceita

Uma classe comum é escrita assim:

```python
class Conta:
    """Docstring opcional."""

    titular: str
    saldo: float

    def __init__(self, titular: str, saldo: float = 0.0) -> None:
        self.titular = titular
        self.saldo = saldo

    def deposita(self, valor: float) -> None:
        self.saldo += valor
```

- O corpo tem uma docstring, `pass`, `...`, campos anotados sem valor e
  `def`. Os campos são os anotados no corpo e os `instance_attributes`
  do `ty`. O checker exige a anotação no corpo, e o `ty` recusa a
  atribuição a um atributo que não existe.
- A ordem dos campos é a da base, como prefixo, e depois a dos campos
  novos da classe, sem repetição.
- A base é uma só, uma classe do programa. Uma dataclass estende só uma
  dataclass, e uma classe comum estende só uma classe comum. Um Enum com
  métodos é aceito, mas não pode ser base.
- Um método recebe `self` como o primeiro parâmetro, sem decorador, e
  aceita defaults e argumentos nomeados como uma função.
- `C(...)` cria o struct com o valor zero de cada campo e chama o
  `__init__` direto, como o jwonder faz com `new` e `<init>`. Uma classe
  sem `__init__` usa o do ancestral mais próximo que tem um, e uma classe
  sem nenhum aceita só `C()`.
- Uma anotação em aspas, como `-> "Ponto"`, vale como a sem aspas.

Um campo lido antes de receber valor lança `AttributeError: 'Conta'
object has no attribute 'saldo'` no CPython. O compilador prova que o
`__init__` atribui cada campo em todos os caminhos antes de ler o campo
e antes de `self` escapar, como a análise de atribuição definida de
Java. `self` escapa numa chamada `self.m()`, num argumento e numa
atribuição a outra variável ou campo. `super().__init__(...)` atribui os
campos que o `__init__` da base prova e é um escape para os campos da
subclasse. Um `__init__` da base que deixa `self` escapar só é aceito
quando toda subclasse dela atribui os próprios campos antes de chamar
`super().__init__`. Assim um método sobrescrito que a base chama nunca
lê um campo da subclasse sem valor. Um campo que a análise não prova é
`Unsupported` (decisão 3).

### Representação

| Tipo do `ty` | `Rep` | Forma local | Forma uniforme |
|---|---|---|---|
| instância de `C` | `Object(C)` | `(ref null $C)`, lido com `ref.as_non_null` | `(ref null eq)` |
| `C \| None` | `ObjectOrNone(C)` | `(ref null $C)` | `(ref null eq)` |
| `object` | `Uniform` | `(ref null eq)` | igual |

Um valor de `Object(Animal)` pode guardar um `Cachorro`, porque
`$Cachorro` é subtipo de `$Animal`. `widen` e `coerce` passam um valor de
`Object(Cachorro)` para `Object(Animal)` sem instrução. `narrow` passa de
`Object(Animal)` para `Object(Cachorro)` com `ref.cast`, depois de um
`isinstance` que o `ty` usa para estreitar.

- Os structs seguem num único rec group, com cada classe depois da base.
  Uma subclasse repete os campos da base como prefixo e declara
  `(sub $Base ...)`, e a base deixa de ser `final`. O jwonder aprendeu
  que dois structs de mesma forma em rec groups separados viram o mesmo
  tipo e quebram o `ref.test` ("A layout is not an identity"). O grupo
  único evita isso.
- **Nenhum `ref.test` e nenhum `ref.cast` nomeia um tipo que não é
  `final`.** Cada classe instanciada que tem subclasses ganha um tipo
  exato, `(sub final $C ...)`, com os mesmos campos. `C(...)` aloca o
  tipo exato, e `$C` fica como o tipo estático e a base. Uma folha já é o
  próprio tipo exato. A medida abaixo mostra por quê.
- Um campo de referência de uma classe comum fica anulável no struct,
  porque o `struct.new` cria o objeto antes do `__init__`, e a leitura
  faz `ref.as_non_null`, que a análise de atribuição definida garante.
  Uma dataclass mantém os campos não anuláveis de hoje.
- Um campo herdado mantém a `Rep` da base. Uma redeclaração com outro
  tipo é `Unsupported`, porque um campo mutável não muda de tipo num
  subtipo.
- O layout sai de `emit/layout.rs`, como no jwonder. O índice de um campo
  vem de uma função sobre a cadeia de bases, e não de uma constante.
- Não há cabeçalho. O objeto não guarda vtable, id de classe nem `hash`.

### A chamada de método

O compilador conhece todas as classes do programa. Depois de baixar
todas as funções alcançadas, um passo novo junta as classes instanciadas
pelos `Expr::New` dessas funções e pelos membros de Enum, separado para
o programa e para os doctests. O jwonder chama isso de RTA. Uma chamada
`x.m()` vira na lowering um nó que ainda não escolheu o corpo, e esse
passo a resolve. Para `x` de tipo estático `S`, os corpos possíveis são
os que `m` tem nas classes instanciadas que são `S` ou descendem de `S`.

- Um corpo só, que é o caso de uma classe sem subclasses e de um método
  que nenhuma subclasse sobrescreve, vira um `call` direto. O jwonder
  chama isso de análise da hierarquia de classes (CHA), e diz que em 10
  dos 13 programas de teste dele nenhuma classe precisou de slot.
- Nenhum corpo quer dizer que nenhum objeto de `S` existe, e a chamada só
  roda com um valor que não pode existir. Ela vira `unreachable`.
- Mais de um corpo vira a chamada de um despachante, gerado uma vez por
  par (método, `S`). Ele testa o tipo exato de cada classe instanciada e
  chama o corpo dela direto, depois do `ref.cast` para esse tipo. O
  último corpo não tem teste. A ordem dos testes não importa, porque cada
  teste é exato.
- `self` tem o tipo da classe que define o método. Uma chamada direta
  nunca faz cast, e o despachante faz um cast por ramo.
- `super().m(...)` é sempre um `call` direto para o corpo que a busca
  acha a partir da base, como o `invokespecial` do jwonder.
- `isinstance(x, C)` de uma classe com subclasses é a cadeia de testes
  contra os tipos exatos das classes instanciadas que descendem de `C`.

O despacho de um valor uniforme, em `print`, `repr`, `==` e na verdade,
testa os mesmos tipos exatos.

A cadeia vale para o programa inteiro da CLI. O REPL compila cada
entrada num módulo próprio, e uma classe definida numa entrada posterior
fica fora de um despachante já compilado. Quando o REPL entrar, cada
objeto vai precisar de um id de classe imutável e da tabela de funções
que o plano do backend já prevê.

### O que vem do jwonder

O jwonder compila Java para WasmGC com a forma do TeaVM. Cada objeto
guarda no campo 0 uma vtable, que é uma struct imutável de `funcref`, um
global constante por classe. Um slot existe só quando o método tem mais
de um corpo, e a chamada faz `struct.get` da vtable, `ref.cast` para o
tipo da vtable da classe que introduziu o método, `struct.get` do slot e
`call_ref`.

O `object-model-proposals.md` do jwonder avaliou a forma deste plano como
a proposta D, sem cabeçalho, com uma cadeia de testes seguida de uma
chamada direta. O jwonder a recusou por dois motivos. O custo cresce com
o número de subtipos em cada ponto de chamada, o que é fatal para a
biblioteca do Java, e o objeto fica sem lugar para o `hash` de
identidade. O mesmo documento diz que a etiqueta `i32` do gleam é a
resposta barata para um conjunto fechado de variantes. Nenhum dos dois
motivos vale aqui. O programa do aluno é fechado e pequeno, e o `hash` de
identidade fica para a fatia 8, quando um campo opcional na raiz da
hierarquia pode guardá-lo só onde o programa o usa, como o jwonder faz
com o `$monitor`.

O alvo de desempenho é o V8, e as medidas abaixo são do deno. O
wasmtime roda os testes e a CLI, e os números dele ficam como nota.

A sondagem `dispatch.wat` do jwonder foi refeita aqui sem o `wasm-opt`,
como o spython emite. O gerador `gen.py` monta um ponto de chamada
`x.m()` num laço de 10^8 voltas, com uma base e N − 1 subclasses, cada
uma com o próprio corpo. No rodízio, o receptor segue o ciclo fixo das
classes. No LCG, um array de 1024 objetos de classes sorteadas é lido
pelo índice que um gerador congruencial linear dá a cada chamada, e
nenhuma sequência se repete. O tempo é em nanossegundos por chamada, o
melhor de 5 execuções, e todas as formas devolvem a mesma soma.

| N | Padrão | cadeia em linha | cadeia em função | id em linha | id em função | vtable |
|---|---|---:|---:|---:|---:|---:|
| 2 | rodízio | 1,4 | 1,4 | 1,4 | 1,5 | 1,8 |
| 4 | rodízio | 1,9 | 2,0 | 1,8 | 1,8 | 2,8 |
| 8 | rodízio | 2,5 | 2,5 | 2,2 | 2,1 | 5,4 |
| 16 | rodízio | 3,3 | 5,2 | 2,2 | 4,0 | 5,9 |
| 2 | LCG | 6,9 | 7,2 | 7,2 | 7,3 | 8,3 |
| 4 | LCG | 10,5 | 10,5 | 10,0 | 10,2 | 11,9 |
| 8 | LCG | 13,0 | 13,1 | 14,6 | 14,1 | 19,8 |
| 16 | LCG | 14,1 | 13,9 | 15,1 | 14,9 | 19,9 |

A cadeia testa os tipos exatos `final`. O id é um campo `i32` imutável
na raiz, com um `br_table` para chamadas diretas. A vtable é a forma do
jwonder, e o corpo dela nem faz o cast que a ponte do jwonder faria. No
LCG, cerca de 6 ns de cada célula são o sorteio e a falha de previsão.

- A vtable perde em todos os casos, de 15% a 50% no LCG. O V8 só põe em
  linha um `call_ref` num ponto quase monomórfico.
- A cadeia e o id empatam, com 10% para um lado ou para o outro, e o id
  custa um campo em todo objeto.
- A cadeia numa função custa o mesmo que em linha até 8 corpos. Com 16
  num padrão previsível, o V8 deixa de pôr a função em linha.
- Um teste contra um tipo sem `final` custa o dobro do teste `final` no
  V8 com 16 classes, e chega a 670 ns no wasmtime. Por isso o plano nunca
  testa um tipo sem `final`.

O compilador emite a cadeia num despachante por par (método, classe
estática), sem cabeçalho no objeto (decisão 2).

O que a fatia leva do jwonder:

- o rec group único, com cada classe depois da base e os campos herdados
  como prefixo;
- a CHA por ponto de chamada sobre o conjunto da RTA, com a chamada
  direta quando só um corpo é possível;
- `self` com o tipo da classe que define o método, e o cast só no
  despacho;
- o objeto criado inteiro antes do `__init__`, que é uma chamada comum;
- `super()` como chamada direta;
- `ref.test` como `isinstance`;
- o layout num arquivo próprio, `emit/layout.rs`;
- os métodos especiais como métodos comuns, que passam pelo mesmo
  despacho;
- nada no objeto que o programa não usa;
- os valores do `_start` partidos em funções de no máximo 64 alocações,
  como o `PER_INIT` do jwonder;
- o `lambda` como uma struct `final` com os campos imutáveis, e o
  `lambda` sem variáveis capturadas como um objeto só, criado uma vez.

### Os métodos especiais

O tipo estático decide o método. O `ty` já recusa um operador sem o
método, então o compilador não lança `TypeError` por falta de método. A
chamada leva o site do operador, e o traceback tem o quadro do método
depois do quadro da linha que usa o operador, com as marcas do CPython.

- `print`, `str` e f-string sem formato chamam `__str__`, e na falta dele
  `__repr__`. `repr` chama `__repr__`. Uma classe comum sem nenhum dos
  dois escreve `<__main__.Conta object at 0x...>` no CPython, com um
  endereço que muda a cada execução. O compilador escreve
  `<__main__.Conta object>`, sem endereço (decisão 4).
- `print` escreve cada argumento e cada separador assim que os converte,
  como o CPython. Um `__str__` que lança no segundo argumento deixa o
  primeiro escrito. Isso muda o `print` de lista da fatia 5, que montava
  o texto inteiro antes de escrever.
- `a == b` chama `a.__eq__(b)`, e `a != b` chama `__ne__`, que por padrão
  nega o `__eq__`. Sem `__eq__`, `==` compara a identidade. O `__eq__`
  recebe `other: object`, que é `Uniform`, e o corpo usa `isinstance`.
- `in`, `count`, `index` e `remove` de uma lista, e a busca numa chave de
  `dict` ou de `set`, testam primeiro a identidade, com `ref.eq`, e só
  depois chamam `__eq__`, como o CPython.
- `a < b` chama `a.__lt__(b)` quando a classe de `a` o define, e senão
  `b.__gt__(a)`. O mesmo vale para `<=`, `>` e `>=`, e uma cadeia como
  `a < b < c` chama cada par.
- `a + b` chama `a.__add__(b)`, e senão `b.__radd__(a)`. O mesmo vale
  para `-`, `*`, `/`, `//`, `%` e `**`. `-a` chama `__neg__`.
- `a += b` chama `__iadd__` quando ele existe, depois `__add__` e depois
  `__radd__` da direita, e atribui o resultado ao alvo. Um `__iadd__`
  sem `return` deixa `None` no alvo, e o `ty` recusa esse caso pelo tipo.
- `len(x)` chama `__len__` e lança `ValueError: __len__() should return
  >= 0` para um resultado negativo. A verdade chama `__bool__`, e na
  falta dele `__len__`, com a mesma mensagem. Sem nenhum dos dois, o
  objeto é verdadeiro.
- `x in c` chama `c.__contains__(x)`. `c[i]` chama `__getitem__`, e
  `c[i] = v` chama `__setitem__`.

O CPython chama o método refletido antes do direto quando a classe
exata da direita é subclasse própria da classe da esquerda. Numa
comparação, basta a subclasse ter o refletido, mesmo herdado. Num
operador aritmético, o refletido da subclasse tem de ser outro corpo,
diferente do da esquerda. O tipo estático não decide esse caso. Quando
alguma subclasse instanciada do tipo estático da esquerda cumpre a
regra, o compilador testa o tipo exato da direita antes da chamada e
chama o refletido primeiro. Com `b` de `B` e `d` de `D(B)`, `b == d`
chama `B.__eq__(d, b)` e `b < d` chama `B.__gt__(d, b)`.

### O `hash`

`set` mostra a ordem da tabela no `print`, e a ordem depende do `hash`.
O `hash` segue o do CPython 3.13, num `i64`.

- `int` é o próprio valor, com −1 trocado por −2. Em 31 bits, o módulo
  2^61 − 1 nunca entra.
- `float` segue `_Py_HashDouble`, num builtin em Rust. `inf` dá 314159.
  O `hash` de `nan` vem da identidade do objeto desde o CPython 3.10, e
  um `float` sem caixa não tem identidade. O compilador dá 0 ao `hash` de
  `nan`, e um `nan` nunca é igual a outro valor num contêiner, nem a ele
  mesmo. Cada `nan` inserido num `set` vira um item novo, e `n in {n}` dá
  `False` (decisão 8).
- `bool` é 0 ou 1. `None` é 4238894112, uma constante desde o 3.12.
- `str` segue o SipHash-1-3 sobre os bytes da forma compacta do CPython,
  com 1, 2 ou 4 bytes por caractere, e 0 para o `str` vazio. O CPython
  sorteia a chave a cada execução, e a ordem de um `set[str]` muda de uma
  execução para outra. Com `PYTHONHASHSEED=0`, a chave são 16 bytes zero.
  O compilador usa essa chave, e os testes rodam o CPython com essa
  variável (decisão 5). O SipHash vem de um crate testado.
- Um membro de Enum tem o `hash` do nome.
- Uma dataclass `frozen` tem o `hash` da tupla dos campos, pelo xxHash
  que o CPython usa nas tuplas.
- Uma classe com `__hash__` chama o método e troca −1 por −2. Uma classe
  com `__eq__` e sem `__hash__`, e uma dataclass que não é `frozen`, não
  têm `hash` no CPython, e o compilador recusa o `set` e a chave de
  `dict` delas.
- Chaves iguais de tipos diferentes ficam com a primeira chave e o
  último valor. `{1: 'a', 1.0: 'b', True: 'c'}` escreve `{1: 'c'}`.

O grupo G fez assim:

- O `hash` é um `Rep::Hash`, um `i64` que só o prelude guarda. O prelude
  o vê como a classe `Hash` dos intrínsecos, e `hash_of(x)` dá o `hash`
  de `x` pela `Rep` de `x`.
- O `hash` de `float` e de `str` vem de builtins em Rust, que um teste
  nativo compara com o CPython. O de `str` usa o crate `siphasher` sobre
  os pontos de código, com 1, 2 ou 4 bytes cada, como a forma compacta.
  O `hash` da tupla de uma dataclass `frozen` sai em linha no módulo.
- Um slot `Key::Hash` dá o `hash` de um objeto. A primeira classe da
  cadeia que diz o `hash` decide. Um `__hash__` do corpo dá o dele, uma
  dataclass `frozen` dá o dos campos, a que não é `frozen` tira o `hash`,
  um Enum dá o do nome, e um corpo com `__eq__` sem `__hash__` tira o
  `hash`.
- O compilador recusa um `set` e uma chave de `dict` cujo tipo pode ter
  um valor sem `hash`. Isso inclui `object`, porque um valor de `object`
  pode ser um `dict`, um `set` ou uma lista.
- O `hash` de um valor uniforme testa a classe exata contra cada classe
  do programa que tem `hash`, e depois os tipos builtin.
- Só o prelude chama `hash_of`, e nenhum programa cria um `dict` ou um
  `set` antes do grupo H. Os testes que comparam a ordem de um `set` com o
  CPython vêm com os grupos H e I.

### `dict` e `set`

`dict` e `set` são classes genéricas do prelude, em Python, e o
compilador faz uma cópia de cada uma por `Rep` das variáveis de tipo,
como faz com as funções de lista (decisão 6). Elas guardam os
dados em listas do prelude, e assim usam o que a fatia 5 já testou.

O mecanismo de hoje não basta, e a parte 6b começa por ele:

- a cópia de `Dict[K, V]` e de `Set[T]`, do prelude, é uma classe do
  programa, e um `dict` é um `Rep::Object` dela. Uma tabela guarda as
  cópias pelos `TypeArg` das variáveis de tipo, e o primeiro uso cria a
  cópia, como cria um `ListId`, com os campos substituídos. Assim o
  `len`, o `in`, o `[]`, o `==` e o `print` passam pelos métodos
  especiais da fatia 6a;
- uma chamada declara cada método da cópia com os tipos dela, e um
  método que o programa não chama não entra no módulo;
- `d.get(k)` chega pelo nome do método, como `method_call` já faz com
  `str` e com lista, porque o `ty` resolve o método no `dict` do
  typeshed. A chamada vai direto ao método, sem despacho;
- só o código do prelude lê um campo da cópia;
- o `for` sobre um `dict` e sobre um `set` é uma forma do compilador,
  com um índice nas listas da cópia, porque `__iter__` fica para a 8;
- o `hash` de uma chave `Uniform` passa por um despacho uniforme.

O que o CPython faz e a cópia segue:

- O `dict` segue o `dict` compacto do CPython, com uma lista de índices
  e listas de `hash`, de chaves e de valores na ordem de inserção.
- O `set` segue o `setobject.c` do CPython 3.13, com as 9 sondagens
  lineares, a perturbação sobre os 64 bits do `hash` e a regra de
  crescimento. `copy`, `set(s)` e `|` passam por `set_merge`, que insere
  numa tabela do tamanho da origem, e por isso uma cópia depois de
  remoções sai em outra ordem. `&` percorre o menor dos dois, e `-` e `^`
  seguem `set_difference` e `set_symmetric_difference`.
- `d[k]` lança `KeyError` com o `repr` da chave, como `KeyError: "it's"`,
  e o traceback só tem o quadro de quem lê, porque uma função do prelude
  não tem quadro. `s.remove(x)` lança `KeyError` também.
- Mudar um `dict` dentro de um `for` sobre ele lança `RuntimeError:
  dictionary changed size during iteration`, ou `dictionary keys changed
  during iteration` quando o tamanho volta ao mesmo. Num `set`, a
  mensagem é `Set changed size during iteration`. O traceback marca o
  iterável na linha do `for`.
- `for k, v in d.items()` é uma forma do `for`, sem tupla. `d.items()`
  fora do `for` é `Unsupported` até a fatia 8.

O grupo H fez assim:

- A classe `Dict` do prelude guarda os índices da tabela, os `hash`es, as
  chaves, os valores e se cada entrada está viva. Uma entrada removida
  fica no seu lugar até a tabela crescer, e a tabela cresce onde o
  CPython faz crescer, inclusive na troca da tabela só de `str` por uma
  geral. Assim as mensagens de mudança dentro do `for` saem como no
  CPython.
- A literal cria o `dict`, guarda cada chave e cada valor na ordem com
  `pend`, e `done` põe os itens numa tabela do tamanho que o CPython dá,
  como `BUILD_MAP`. O `hash` de cada chave vem depois de todos os itens.
- O `KeyError` ganhou uma classe. Seu texto é o `repr` do argumento, e o
  prelude escreve `raise KeyError(key)`.
- O tipo de `d.get(k, x)` e de `d.pop(k, x)` depende de `x`. A chamada vai
  a `get_or` e a `pop_or`, métodos genéricos que a lowering copia pelo tipo
  de `x`, e um `None` literal vai a `get_or_none` e a `pop_or_none`.
- O `for` guarda o seu lugar num objeto de `DictIter`, outra classe do
  prelude, cujo `next` segue `dictiter_iternextkey`.
- O `repr` de um `dict` dentro dele mesmo escreve `{...}`, pela pilha dos
  objetos cujo `repr` roda, a mesma das dataclasses.
- `__eq__` segue `dict_equal`. Uma cópia fica fora das classes de um valor
  uniforme, e o emissor compara dois valores uniformes de uma cópia com o
  `__eq__` dela. Sem isso, o tipo de `dict[str, object]` mudava conforme
  as cópias criadas antes dele.
- A comparação de dois `dict`s de tipos diferentes, como `dict[str, int]`
  e `dict[str, float]`, é `Unsupported`. Dentro de `object`, ela dá
  `False` mesmo quando o CPython dá `True`.

O grupo I fez assim:

- A classe `Set` do prelude guarda a tabela em três listas, com o item, o
  `hash` e o estado de cada slot, que está vazio, em uso ou é de um item
  removido. A busca, a inserção, o crescimento e `set_merge` seguem o
  `setobject.c`. Um programa rodou a classe no CPython, com as
  intrínsecas trocadas por funções de Python, e conferiu a ordem de
  milhares de `set`s aleatórios contra a dos `set`s do CPython.
- Uma comparação que tira o item do slot ou troca a tabela recomeça a
  busca, como no CPython. Um item sem referência, como um `int`, não tem
  `__eq__` que mude o `set`, e `same(k, k)` diz se o item tem referência.
- A literal põe os itens na ordem do código, como `BUILD_SET`. O CPython
  compila uma literal de mais de 2 constantes, e o iterável constante de
  um `for`, como um `frozenset`. Ele refaz o `frozenset` na ordem dos
  slots quando guarda a constante, e de novo quando cria o code object, se
  algum item é um `str` que outro objeto já internou. Essa última regra
  depende das palavras que o CPython interna na partida, e por isso o
  compilador não segue o `frozenset`. A literal `{5, 3, 40, -2, 17}` sai
  em outra ordem que a do CPython.
- `set(xs)` põe os itens de uma lista em ordem, e `set(s)` e `s.copy()`
  passam por `set_merge`. Outro iterável é `Unsupported`.
- `|`, `&` e `^` chamam `__or__`, `__and__` e `__xor__` e os métodos
  refletidos, e uma classe do programa pode defini-los também. `|=`,
  `&=`, `-=` e `^=` mudam o próprio `set`, como no CPython.
- O `for` guarda o seu lugar num objeto de `SetIter`, cujo `next` segue
  `setiter_iternext`.
- O builtin `hash` dá como `int` o mesmo `hash` que o prelude usa. Um
  `hash` fora dos 31 bits para o programa, como as outras contas de
  `int`, e o de um `str` quase sempre fica fora.
- `&`, `|` e `^` de dois `int`s são uma instrução só, e o prelude usa
  `& mask` na sondagem da tabela.
- Uma literal sem anotação que um `for` sobre ela mesma muda, como em
  `s = {1, 2, 3}` seguido de `for x in s: s.add(x + 3)`, tem o tipo
  `set[Divergent]` no checker, e o programa precisa anotar `s: set[int]`.
  O mesmo acontece com uma lista e `append`.

### Funções como valores

Um valor de função tem uma `Rep` por assinatura, `Func(sig)`, com as
`Rep` dos parâmetros e do resultado, e a anotação `Callable[[int], int]`
dá a mesma `Rep`. O valor é uma struct `final` com a função num campo
`funcref` imutável e as variáveis capturadas em campos imutáveis. A
chamada lê o campo e faz `call_ref`. A sondagem `funcref.wat` do jwonder
mede 2,1 ns no V8 para essa forma, contra 2,5 ns de um índice numa
tabela com `call_indirect` (decisão 7). O código recebe a struct como
primeiro parâmetro e faz o cast para a struct da própria função, que é
`final`.

- A chamada de um valor de função fica numa função só do emissor, para
  trocar de forma se outra medida pedir. No wasmtime, o `call_ref` custa
  cerca de 41 ns, e o `call_indirect` 7 ns.
- O nome de uma função do módulo vira um objeto só, criado uma vez num
  global, por `def`, porque o nível 4 aceita a segunda definição.
- Um `lambda` e um `def` dentro de uma função capturam as variáveis que
  leem. Uma variável capturada que alguém atribui depois da captura fica
  numa célula. A variável de um `for` conta como atribuída depois, e
  `fs.append(lambda: i)` num laço vê o último valor, como no CPython.
- O quadro de um `lambda` no traceback se chama `<lambda>`, e o de um
  `def` interno tem o nome dele.
- A chamada de um valor de função pode lançar, como uma chamada de função
  que o compilador não conhece.

O grupo J fez assim:

- O valor de uma função do módulo vive num global, que um `struct.new`
  constante cria, e cada `def` tem o seu. Um `lambda` e um `def` interno
  fazem um valor novo a cada execução, com `ref.func`, e dois valores do
  mesmo `lambda` não são iguais, como no CPython. `==` e `is` de funções
  comparam a identidade.
- A struct do tipo de função não é `final`. A struct de uma função que
  captura variáveis é um subtipo `final` dela, e o código faz o cast uma
  vez, na entrada.
- Uma variável capturada guarda uma cópia quando tem uma ligação só, fora
  de laço, e tem valor em toda captura. Qualquer outra vive numa célula,
  com o valor e um flag. A leitura de uma célula sem valor lança o
  `NameError` de variável livre do CPython, e a sugestão olha também as
  capturas, depois das locais, na ordem dos nomes.
- Um `def` interno que lê o próprio nome, ou o de um `def` que vem depois,
  acha o nome numa célula.
- `from typing import Callable` passa, e `Callable` só serve na anotação.
- `print`, `str` e `repr` de uma função são `Unsupported`, porque o
  CPython escreve o endereço. Um default num `def` interno também é,
  porque a chamada de um valor passa todos os argumentos. Um `lambda`
  num doctest e no corpo de uma classe são `Unsupported`.
- Uma função não entra numa união, e `Callable | None` é `Unsupported`.

### As compreensões

- Uma compreensão de lista, de `set` ou de `dict` vira um laço em linha,
  como o CPython 3.12 faz (PEP 709). As variáveis dela não aparecem fora,
  e o traceback não tem quadro próprio.
- Uma expressão geradora dentro de `sum`, `any`, `all`, `min`, `max`,
  `sorted` ou `join` avalia o primeiro iterável no quadro de quem chama.
  Uma função gerada com o nome `<genexpr>` dá o próximo item a cada
  chamada, com o estado do laço nos parâmetros e nos resultados, e roda
  o elemento e as condições. O consumidor roda no quadro de quem chama, e
  um erro dele, como `max() iterable argument is empty`, não tem o quadro
  `<genexpr>`. `any` e `all` param no primeiro resultado que decide.
- O `sum` de `float` segue a soma compensada do CPython 3.12, e o de
  `int` testa os 31 bits a cada passo, como o `+`.
- `max` e `min` guardam o primeiro extremo e comparam com `item > atual`
  e `item < atual`, e por isso `max([nan, 1.0])` é `nan` e
  `max([1.0, nan])` é `1.0`, como no CPython.

O grupo K fez assim:

- Uma compreensão é um bloco de comandos dentro de uma expressão. Cada
  nome que ela liga é uma variável escondida, que não entra na sugestão de
  um `NameError`, como no CPython 3.13. Um `lambda` dentro de uma
  compreensão que lê um nome dela é `Unsupported`, porque os valores do
  `lambda` dividem a célula do nome.
- O `set` e o `dict` de uma compreensão recebem cada valor pelo `add` e
  pelo `__setitem__` da cópia do prelude, e o traceback de um erro no
  `hash` marca o valor, ou a chave e o valor.
- A expressão geradora é uma função `<genexpr>` com o parâmetro `.0`, que
  recebe a primeira parte do primeiro iterável. As outras partes de um
  `range` vão como capturas sem nome, e todas rodam em ordem no quadro de
  quem chama. Os nomes dos laços são locais do quadro, e a sugestão de um
  `NameError` lá dentro acha `.0` e esses nomes, como no CPython. O
  `RecursionError` da criação do gerador marca os parênteses dele.
- Para `sum`, `min`, `max`, `sorted` e `join`, a função do gerador põe os
  valores numa lista, e o consumidor roda depois, no quadro de quem chama.
  `any` e `all` param no primeiro valor que decide. `min` e `max` de um
  gerador de objetos, `any` e `all` de um gerador de objetos ou de uniões,
  e `min` e `max` de um gerador com `key` são `Unsupported`, porque a
  ordem das escritas de um método mudaria.
- `sum`, `min`, `max`, `any` e `all` de uma lista estão em
  `consumers.py`, e o `sum` de `float` dá o `int` 0 para a lista vazia,
  como no CPython.
- `sorted` aceita uma lista ou uma expressão geradora. O de um `str`, de
  um `set` ou de um `dict` é `Unsupported`.
- O checker recusa `xs.sort()`, porque o tipo do método é um `Overload`, e
  recusa também um `lambda` como `key` de `min` e `max`, cujo parâmetro
  ele não deduz. O compilador já traduz `xs.sort(key=..., reverse=...)`.
- O prelude não tem `try`. Quando um `__lt__` ou a `key` lança no meio do
  `sort`, a lista fica vazia, e o CPython a devolve com os itens. A
  diferença só aparece num doctest que segue depois do erro.

### O `sort`

`sort` e `sorted` portam o Timsort do CPython para o prelude, em Python.
Uma ordem estável qualquer daria o mesmo resultado para uma ordem total,
mas o Timsort dá o mesmo resultado do CPython também com `nan` e com um
`__lt__` incoerente, e chama `__lt__` o mesmo número de vezes.
`sorted([3.0, nan, 1.0, 2.0])` dá `[3.0, nan, 1.0, 2.0]`.

- `key` roda uma vez por item, na ordem da lista, antes de ordenar. Se
  `key` lança, a lista fica como estava.
- `reverse=True` inverte, ordena e inverte de novo, como o CPython, e
  assim fica estável.
- O port em `sort.py` segue o `listobject.c` do CPython 3.13, com a
  política de merge do powersort e o `count_run` que estende uma corrida
  descendente com iguais. Um teste compara o número e a ordem das
  comparações com o CPython em listas de até 1500 itens, com galope e com
  um `__lt__` incoerente.
- A lista fica vazia durante a ordenação, como no CPython, e uma lista que
  ganha itens nesse meio-tempo lança `list modified during sort`.

## Etapas

Cada linha é um commit, e cada commit passa pelo portão rápido.

**6a. A. Preparação**

1. `compiler: split the values of _start into functions of 64 allocations`
2. `compiler: move the layout of the classes to emit/layout.rs`
3. `compiler: give object the uniform rep`, com a leitura estreitada de
   um valor uniforme

**6a. B. Classes com métodos**

4. `compiler: accept a class with annotated fields and __init__`, com a
   criação do struct, os campos anuláveis e a chamada direta do
   `__init__`, aceitando só um `__init__` que atribui os campos em
   comandos simples antes de qualquer outro
5. `compiler: prove that __init__ assigns each field before a read`,
   com a análise de atribuição definida e os escapes de `self`
6. `compiler: call a method of a class directly with self`, com defaults,
   argumentos nomeados e a anotação em aspas do `ok_class.py`
7. `compiler: accept methods in a dataclass`
8. `compiler: accept methods in an Enum`
9. `compiler: write each argument of print as soon as it is converted`
10. `compiler: write an object with __str__ and __repr__`

**6a. C. Herança**

11. `compiler: accept a class with one base of the program`, com o
    prefixo dos campos, a base sem `final`, o tipo exato, os métodos
    herdados e o `Unsupported` de um método que uma subclasse sobrescreve
12. `compiler: call the method of a base with super()`
13. `compiler: widen an object to its base and narrow it with a cast`
14. `compiler: resolve method calls after the instantiated classes`, com
    o nó que ainda não escolheu o corpo, o passo da RTA e a CHA, e um
    teste dos conjuntos
15. `compiler: dispatch a method call on the exact type of the object`,
    que tira o `Unsupported` do commit 11
16. `compiler: dispatch print, repr and == of a uniform value on exact
    types`, com o `dataclass_eq`, o `classes_in` e o
    `uniform_dataclasses` de uma classe com subclasses
17. `compiler: accept a dataclass that extends a dataclass`

**6a. D. Métodos especiais**

18. `compiler: compile isinstance with a class or a builtin type`, com o
    `bool` como `int`
19. `compiler: compile == and != with __eq__ and __ne__`
20. `compiler: test identity before __eq__ in in, count, index and remove`,
    com as listas de itens uniformes que a fatia 5 deixou de fora
21. `compiler: compile the order operators with __lt__ and the others`,
    com a cadeia de comparações de objetos
22. `compiler: call the reflected method first for a subclass operand`
23. `compiler: compile the arithmetic operators with special methods`
24. `compiler: compile += with __iadd__`
25. `compiler: compile len and the truth of an object`
26. `compiler: compile in, [] and []= with special methods`

**6a. E. O resto do nível 4**

27. `compiler: take the truth of None, a str or None and a union`
28. `compiler: compile and and or with operands of two types`
29. `compiler: accept a second def or class of the same name`
30. `docs: record the classes of slice 6 in the plan of the backend`,
    com as medidas do despacho, a troca da `$VT` pelos tipos exatos e o
    `isinstance` por `ref.test`

**6b. F. O mecanismo**

31. `compiler: copy a generic prelude class for the reps of its types`,
    com a tabela das cópias e os métodos declarados na primeira chamada
32. Saiu. A cópia já dá os tipos aos seus métodos, e nenhuma função
    fora da classe recebe um `dict`.
33. `compiler: route a method of dict and set to the prelude class`

**6b. G. O `hash`**

34. `compiler: run CPython with PYTHONHASHSEED=0 in the tests`
35. `compiler: hash an int, a float, a bool and None as CPython does`
36. `compiler: hash a str with SipHash-1-3`
37. `compiler: hash an Enum member and a frozen dataclass`
38. `compiler: hash an object with __hash__`
39. `compiler: reject a set or a dict key of an unhashable class`
40. `compiler: hash a uniform value`

**6b. H. O `dict`**

O grupo começou por dois commits que o plano não tinha,
`compiler: raise KeyError with the repr of its argument` e
`compiler: add the intrinsics hash_slot, hash_shift and hash_eq`.

41. `compiler: compile a dict literal, len, [] and []=`
42. `compiler: write a dict in print, repr and str`
43. `compiler: compile in, get, pop, clear and copy of a dict`
44. `compiler: compile for over a dict, its keys and its values`
45. `compiler: compile for k, v in d.items()`
46. `compiler: compile == and != of dicts`

**6b. I. O `set`**

47. `compiler: compile a set literal, set(), len, in, add and remove`,
    com `discard`
48. `compiler: write a set in print, repr and str`
49. `compiler: compile for over a set`
50. `compiler: compile copy, set of a list and | of sets`
51. `compiler: compile &, -, ^ and the comparisons of sets`
52. `docs: record dict and set in the plan of the backend`, com as
    medidas

**6c. J. Funções como valores**

53. `compiler: compile the name of a function as a value`, com a
    `Rep::Func` e a anotação `Callable`
54. `compiler: call a value of a function type`
55. `compiler: compile a lambda that captures no variable`
56. `compiler: capture the variables of the enclosing function`, com a
    célula
57. `compiler: compile a def inside a function`

**6c. K. Compreensões e consumidores**

58. `compiler: compile a list comprehension in line`
59. `compiler: compile set and dict comprehensions`
60. `compiler: compile sum, min, max, any and all of a list`
61. `compiler: compile a generator expression in a consumer`
62. `compiler: compile sort and sorted with key and reverse`, com a forma
    geradora de `sorted`
63. `compiler: compile min and max with key`

**6c. L. Fecho**

64. `compiler: check the corpus at level 4`, com `COVERED` em
    `Level::Classes` e os arquivos de `cli/tests/inputs` no nível que o
    teste diz
65. `docs: record the slice 6 in the plan of the backend`

## Testes

- Cada commit traz os testes do que ele compila, em
  `compiler/tests/run/`, com o CPython como oráculo do stdout, do stderr
  e do código de saída, no wasmtime e no deno.
- Cada forma recusada tem um teste de `Unsupported` com a mensagem.
- Casos de borda com o CPython como oráculo:
  - um método sobrescrito em dois níveis, chamado por um valor do tipo da
    raiz, de cada classe intermediária e de uma folha, com a classe do
    meio instanciada e não instanciada;
  - uma raiz que o programa nunca instancia;
  - `super()` em dois níveis, no `__init__` e num método comum, e um
    `__init__` da base que chama um método sobrescrito;
  - o traceback de uma exceção dentro de `__eq__`, de `__lt__`, de
    `__add__`, de `a += a`, de `__len__` chamado por `len` e por `if`, e
    de `__str__` chamado pelo `print` depois de um primeiro argumento;
  - `b == d` e `b < d` com `d` de uma subclasse, e `a + b` com só
    `__radd__`;
  - `e in [e]`, `[e].count(e)` e `[e].index(e)` com um `__eq__` que
    sempre dá `False`;
  - `isinstance(True, int)` e as chaves `1`, `1.0` e `True` num `dict` e
    num `set`;
  - o `print` de um `set` de `int` que cresce, de um `set` com `-1` e
    `-2`, de `str` com `PYTHONHASHSEED=0`, de `a | b` e `b | a`, e de uma
    cópia depois de remoções;
  - as três mensagens de mudança dentro do `for`, com a mudança na última
    volta;
  - o `KeyError` de uma chave `str` com aspas simples e de uma chave
    objeto;
  - um `lambda` que captura uma variável atribuída depois, e um `lambda`
    criado num `for`;
  - uma compreensão aninhada que lança, uma expressão geradora que lança
    dentro de `sum` e de `any`, e `max` de uma expressão geradora vazia;
  - o `sort` com `nan`, com `key=` que lança, com chaves iguais e com
    `reverse=True`;
  - o despacho com 8 e 16 classes.

## Medidas

O commit 30 registra a medida do despacho e mede um programa com uma
hierarquia de quatro classes. O commit 52 mede 10^6 inserções e buscas
num `dict[int, int]` e num `set[str]`, no wasmtime e no deno, contra o
CPython, e o tamanho do módulo de cada cópia. O commit 65 mede o `sort`
de 10^6 `int`, uma chamada por `lambda` num laço com um alvo só, nos dois
motores, e o tempo do `wasmtime compile` de um programa com 500 literais
de `str`.

## Riscos

- **A análise de atribuição definida.** Um erro nela deixa um
  `ref.as_non_null` falhar com um trap, sem traceback. Ela tem de recusar
  todo caso que não prova, inclusive o escape por um método sobrescrito.
- **O conjunto das classes instanciadas.** Uma classe criada só dentro de
  uma cópia do prelude, num default ou num doctest tem de entrar no
  conjunto. Um erro faz o despachante chamar o corpo errado.
- **Um tipo sem `final` num teste.** Um `ref.test` que escapa da regra
  custa o dobro no V8, e centenas de nanossegundos no wasmtime, sem
  mudar nenhuma saída.
  Um teste do emissor confere que nenhum `ref.test` nem `ref.cast` nomeia
  um tipo que não é `final`.
- **O tempo de compilação.** O Cranelift cresce como n^2,8 com as
  alocações numa função. Cada objeto criado no `_start` conta, e o commit
  1 só protege o `_start`.
- **O `hash` de `str`.** Ele depende da forma compacta do CPython. Um
  erro aparece só na ordem de um `set`.
- **O tamanho do módulo.** Cada `dict[K, V]` e cada `set[T]` é uma cópia
  das classes do prelude.
- **O Timsort.** O port é longo, e a parte de galope só roda com
  sequências grandes e parcialmente ordenadas. Os testes precisam de
  casos que a alcancem.

## Em aberto

- Um id `i32` imutável na raiz e o `br_table`, se um programa tiver
  muitos corpos num ponto de chamada. A medida empata com a cadeia até 16.
  O id em pré-ordem daria também o `isinstance` por intervalo, a proposta
  B do jwonder.
- O REPL, com o id de classe e a tabela de funções para as classes de
  entradas posteriores.
- A especialização pelo código de um argumento função.
- O `hash` de identidade, quando a fatia 8 aceitar `set` de uma classe
  sem `__eq__`.
- O `.0` da função de uma expressão geradora, quando a fatia 8 trouxer
  `__iter__` e o gerador como valor. Ele passa a ser o iterador do
  primeiro iterável, como no CPython, e o `for` passa a iterar sobre um
  iterador já criado. A tabela `substitutes`, que hoje troca as partes
  do primeiro iterável pelos valores de fora, sai nessa hora.
- `sum`, `min` e `max` de uma expressão geradora, que hoje põem os
  valores numa lista antes de consumi-los. Com o gerador como iterador,
  na fatia 8, as funções de `consumers.py` passam a consumir o iterador,
  como o CPython faz.

## O que a revisão mudou

Quatro revisores leram a primeira versão, um técnico, um da semântica do
CPython, um do escopo e um da fidelidade ao jwonder.

- O teste contra um tipo sem `final` custa 670 ns com 16 classes no
  wasmtime. Cada classe instanciada com subclasses ganhou um tipo exato,
  e o atalho da raiz de uma subárvore saiu.
- O plano passou a citar a proposta D do jwonder, que é a forma escolhida
  aqui, e os motivos da recusa lá.
- As classes instanciadas vêm de um passo depois da lowering, a RTA, e a
  chamada sem corpo virou `unreachable`.
- O código que supõe a classe estática exata ganhou commits próprios
  (13 e 16).
- A análise de atribuição definida trata o escape por um método
  sobrescrito.
- O commit 1 parte o `_start`, pela medida de compilação do jwonder.
- A regra do método refletido segue o CPython, com o teste do tipo exato
  em tempo de execução.
- A identidade vem antes do `__eq__` nos contêineres.
- O `print` escreve cada argumento logo.
- A expressão geradora deixa o primeiro iterável e o consumidor no quadro
  de quem chama.
- O `set` porta também a cópia, a união, a interseção e as diferenças.
- O `hash` de `nan`, a terceira mensagem de mudança no `for`, o `bool`
  como `int` nas chaves e no `isinstance`, a regra do −1 no `__hash__`, o
  `max` com `nan` e o `reverse` do `sort` entraram.
- O mecanismo das classes genéricas do prelude ganhou três commits
  (31 a 33) e o `hash` uniforme (40).
- `object` como parâmetro ganhou um commit (3).
- Os valores de função perderam os adaptadores, que foram para a 8.
- O portão do corpus foi para o fim da 6c, e os commits que o
  compilador já cumpria (`bool` numa conta) saíram.
- Os commits com mais de uma mudança foram partidos.

## Decisões

O usuário decidiu estes pontos em 12/nov/2026, uma pergunta por vez.

1. A fatia se divide em 6a, 6b e 6c, nessa ordem.
2. O alvo de desempenho é o V8, e o wasmtime fica para os testes e a
   CLI. O despacho é a cadeia de `ref.test` contra tipos exatos `final`,
   num despachante por par (método, classe estática), sem cabeçalho no
   objeto. O usuário pediu a sondagem refeita com o despachante em linha
   e em função, com o id e com um padrão que não se repete, antes de
   decidir.
3. Um campo que a análise não prova atribuído é `Unsupported`.
4. O `print`, o `str`, o `repr` e a f-string de um objeto sem `__str__`
   e sem `__repr__` escrevem `<__main__.Conta object>`, sem endereço.
5. O `hash` de `str` é o SipHash-1-3 com a chave de `PYTHONHASHSEED=0`,
   e os testes rodam o CPython com essa variável.
6. `dict` e `set` são classes genéricas do prelude, em Python.
7. Um valor de função guarda a função num campo `funcref`, e a chamada é
   `call_ref`.
8. O `hash` de `nan` é 0, e um `nan` nunca é igual num contêiner, a
   mesma divergência da decisão 4 da fatia 5.
