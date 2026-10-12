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

O spython restringe as construções de Python disponíveis com base no nível de ensino. O nível padrão é 5, a linguagem inteira.

| Nível | Nome      | Construções adicionadas                                              |
|-------|-----------|----------------------------------------------------------------------|
| 0     | Funções   | `def`, `return`, escalares, indexação de `str`                       |
| 1     | Seleção   | `if`/`elif`/`else`, `x if c else y`                                  |
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

- **Condições booleanas** — o teste de `if`/`elif`/`while`/ternário/`assert`,
  o guard de um `case` e os operandos de `and`/`or`/`not` precisam ter tipo
  `bool`. "Truthiness" implícita (`if x:` com `x: int`) é rejeitada.
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

`:time` — Avalia uma expressão e mostra o tempo de execução:

```
>>> :time 2 ** 100000
... número grande ...
Time: 5 ms
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
spython test arquivo.py
```

Os doctests são verificados quanto a tipos antes de serem executados,
seguindo as mesmas regras do nível de ensino atual.

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
$ spython test --level 5 teste.py
Running tests...
  dobro
2 tests, 2 successes, 0 failures and 0 errors.
```

Os exemplos seguem as regras do módulo `doctest` do Python. A saída
esperada termina numa linha em branco ou no próximo `>>>`, então uma
linha em branco dentro da saída é escrita como `<BLANKLINE>`. Um exemplo
que lança uma exceção espera a última linha do traceback, sozinha ou depois
de uma linha `Traceback (most recent call last):`:

```python
def primeiro(xs: list[int]) -> int:
    """
    >>> primeiro([7, 8])
    7
    >>> primeiro([])
    Traceback (most recent call last):
    ...
    IndexError: list index out of range
    """
    return xs[0]
```

Cada exemplo tem um único comando. Os exemplos de uma docstring veem os
nomes do módulo e os nomes que os exemplos anteriores definem, e um nome
definido numa docstring não existe nas outras. O executor não tem opções,
então uma diretiva `# doctest:` é um erro. Um doctest que nunca seria
executado também é um erro. Isso acontece numa função definida dentro de
outra função, numa definição sob uma verificação de `__name__` e num
dicionário `__test__`.

Use `--verbose` para ver todos os testes (não apenas as falhas):

```sh
spython test --level 5 --verbose teste.py
```


# Verificação

Para verificar os tipos de um arquivo sem executá-lo:

```sh
spython check arquivo.py
```

Se não houver erros, nada é impresso. Caso contrário, os erros são mostrados.


# Formatação

Para formatar arquivos Python:

```sh
spython format arquivo.py
```

Ou para formatar todos os arquivos `.py` em um diretório:

```sh
spython format diretorio/
```

Para saber quais arquivos não estão formatados, sem alterar nenhum deles:

```sh
spython format --check diretorio/
```

Cada arquivo que não está formatado é nomeado, e o comando termina com
status de falha. É o que um job de integração contínua executa.

Sem caminho, o `format` lê o código da entrada padrão e escreve o código
formatado na saída padrão. É o que um editor chama para formatar o arquivo
que está aberto:

```sh
spython format < arquivo.py
```


# Comandos

| Comando | Descrição |
|---------|-----------|
| `spython` | Modo interativo (REPL) |
| `spython repl [arquivo]` | Modo interativo (REPL) |
| `spython run arquivo` | Executa o arquivo com verificação de tipos |
| `spython check arquivo` | Verifica os tipos sem executar |
| `spython test arquivos` | Executa os doctests |
| `spython format [--check] caminhos` | Formata o código, ou diz o que não está formatado |
| `spython help [comando]` | Exibe a ajuda do programa ou de um comando |

# Opções

| Opção | Descrição |
|-------|-----------|
| `--level N` | Nível de ensino (0-5, padrão: 5) |
| `--verbose` | Mostra todos os testes (com `test`) |
| `--version` | Exibe versão |
