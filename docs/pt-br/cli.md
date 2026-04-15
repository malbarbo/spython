# Executando um arquivo

Para executar um arquivo Python com verificação de tipos:

```sh
spython run arquivo.py
```

O spython verifica as anotações de tipo e as construções permitidas pelo nível de ensino antes de executar. Se houver erros, a execução é bloqueada e os erros são exibidos.

```python
# ola.py
def main() -> None:
    print("Olá mundo!")

main()
```

```sh
$ spython run --level 5 ola.py
Olá mundo!
```


## Níveis de ensino

O spython restringe as construções de Python disponíveis com base no nível de ensino. O nível padrão é 0 (mais restrito).

| Nível | Nome      | Construções adicionadas                                              |
|-------|-----------|----------------------------------------------------------------------|
| 0     | Funções   | `def`, `return`, escalares, indexação de `str`                       |
| 1     | Seleção   | `if`/`elif`/`else`                                                   |
| 2     | Tipos de usuário | `class` (Enum / `@dataclass`), `match`                               |
| 3     | Repetição | literais de `list`, `for`, `while`, `+=`                             |
| 4     | Classes   | `class` com métodos, `dict`/`set`, compreensões, `lambda`            |
| 5     | Completo  | irrestrito (apenas anotações são exigidas)                           |

Para especificar o nível:

```sh
spython run --level 2 arquivo.py
```

Por exemplo, no nível 0, o uso de `if` gera um erro:

```python
# cond.py
def f(x: int) -> int:
    if x > 0:
        return x
    return 0
```

```sh
$ spython run --level 0 cond.py
error[forbidden-selection]: `if` statement is not allowed at level 0 (Functions)
```


## Restrições adicionais nos níveis 0–3

Além de controlar quais construções estão disponíveis, os níveis 0 a 3
aplicam restrições extras pensadas para evitar erros comuns de iniciantes.
Todas elas são liberadas no nível 4.

- **Condições booleanas** — o teste de `if`/`elif`/`while`/ternário/`assert`
  e os operandos de `and`/`or`/`not` precisam ter tipo `bool`. "Truthiness"
  implícita (`if x:` com `x: int`) é rejeitada.
- **Sem `bool` em aritmética** — valores `bool` não são aceitos como
  operandos de `+`, `-`, `*`, `/`, `//`, `%`, `**`, atribuição aumentada ou
  unário `+`/`-`. Isso evita bugs de aluno como `x + True`, que funciona
  silenciosamente porque `bool` é subclasse de `int`.
- **Sem comparações encadeadas** — `a < b < c` e `a == b != c` precisam ser
  desmembradas com `and`, já que a semântica de comparação encadeada do
  Python é confusa para iniciantes.
- **Sem expressão como statement** — uma expressão cujo resultado é
  descartado (qualquer coisa que não seja uma chamada de função, uma
  docstring ou `...`) é rejeitada, capturando casos como `x + 1` onde o
  aluno esqueceu de atribuir ou imprimir.
- **Sem valores padrão em parâmetros** — parâmetros de função não podem ter
  valores padrão; todo argumento deve ser passado explicitamente.


## Anotações obrigatórias

Em todos os níveis, o spython exige anotações de tipo completas:

- Todo parâmetro de função (exceto `self`/`cls`) precisa de anotação
- Toda função precisa de anotação de retorno
- Toda atribuição no corpo de uma classe precisa de anotação

```python
# sem_anotacao.py
def dobro(x):
    return x * 2
```

```sh
$ spython run sem_anotacao.py
error[missing-parameter-annotation]: Parameter `x` is missing a type annotation
error[missing-return-annotation]: Function `dobro` is missing a return type annotation
```

A versão correta:

```python
# dobro.py
def dobro(x: int) -> int:
    return x * 2

print(dobro(5))
```

```sh
$ spython run --level 5 dobro.py
10
```


# Modo interativo (REPL)

Para entrar no modo interativo:

```sh
spython
```

No REPL, você pode digitar expressões e definições:

```
>>> 1 + 2
3
>>> x: int = 10
>>> x * 2
20
```

O REPL verifica tipos e anotações a cada entrada. Definições sem anotação são rejeitadas:

```
>>> def f(x): return x
error[missing-parameter-annotation]: Parameter `x` is missing a type annotation
error[missing-return-annotation]: Function `f` is missing a return type annotation
```

A versão correta:

```
>>> def f(x: int) -> int:
...     return x * 2
...
>>> f(5)
10
```

Para especificar o nível de ensino no REPL:

```sh
spython repl --level 3
```

Também é possível carregar um arquivo, tornando as definições disponíveis no REPL.
Por exemplo, dado o arquivo `dobro.py`:

```python
def dobro(x: int) -> int:
    """
    >>> dobro(0)
    0
    >>> dobro(3)
    6
    """
    return x * 2
```

Podemos usar a função `dobro` no REPL:

```sh
spython repl --level 5 dobro.py
```

```
>>> dobro(5)
10
>>> dobro(3) + 1
7
```

O arquivo é verificado (tipos e doctests) antes de ser carregado.


## Comandos do REPL

`:help` — Mostra os comandos disponíveis.

`:type` — Mostra o tipo estático de uma expressão sem avaliá-la:

```
>>> :type 1 + 2
int
>>> :type [1, 2, 3]
list[int]
>>> :type "hello"
str
```

`:level` — Mostra ou altera o nível de ensino:

```
>>> :level
level 0 - Functions
>>> :level 3
level 3 - Repetition
```

Se o código já digitado não for compatível com o novo nível, a mudança é rejeitada.

`:theme` — Mostra ou altera o tema de syntax highlighting (`light` ou `dark`):

```
>>> :theme
dark
>>> :theme light
light
```

As preferências de tema e nível são salvas no diretório de configuração do usuário.

`:quit` — Sai do REPL (ou `Ctrl+d`).


# Testes (doctests)

Para executar os doctests de um arquivo:

```sh
spython check arquivo.py
```

Os testes são escritos como doctests nas docstrings:

```python
# teste.py
def dobro(x: int) -> int:
    """
    >>> dobro(0)
    0
    >>> dobro(3)
    6
    """
    return x * 2
```

```sh
$ spython check --level 5 teste.py
Running tests...
2 tests, 2 successes, 0 failures and 0 errors.
```

Use `--verbose` para ver todos os testes (não apenas as falhas):

```sh
spython check --level 5 --verbose teste.py
```


# Formatação

Para formatar arquivos Python:

```sh
spython format arquivo.py
```

Ou para formatar todos os arquivos `.py` em um diretório:

```sh
spython format diretorio/
```


# Comandos

| Comando | Descrição |
|---------|-----------|
| `spython` | Modo interativo (REPL) no nível 0 |
| `spython repl [arquivo]` | Modo interativo (REPL) |
| `spython run arquivo` | Executa o arquivo com verificação de tipos |
| `spython check arquivos` | Executa os doctests |
| `spython format caminhos` | Formata o código |
| `spython help` | Exibe ajuda |

# Opções

| Opção | Descrição |
|-------|-----------|
| `--level N` | Nível de ensino (0-5, padrão: 0) |
| `--verbose` | Mostra todos os testes (com `check`) |
| `--version` | Exibe versão |
