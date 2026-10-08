# Plano: Execucao Passo a Passo

## Objetivo

Adicionar um modo de execucao passo a passo ao `spython` com estas propriedades:

- o programa executa uma vez e gera uma timeline de estados
- o usuario pode avancar e retroceder sem reexecutar
- a UI mostra linha atual, saida, pilha de execucao e memoria
- o trace nao entra em modulos pre-definidos
- o trace funciona apenas para programas de um unico arquivo
- se o arquivo do usuario importar outro arquivo do usuario, o modo deve recusar a execucao

Este documento consolida o estudo de viabilidade e a proposta de implementacao para outro agente continuar.

## Conclusao de viabilidade

A implementacao e viavel.

Motivos principais:

- a execucao web ja passa por um `ReplState` central no engine
- o caminho de execucao ja esta concentrado no `engine`
- a captura de `stdout/stderr` ja existe sob feature `capture`
- a fork de RustPython usada pelo projeto ja expoe tracing por frame/linha e acesso a `f_locals`, `f_globals`, `f_back` e `f_lineno`

Arquivos relevantes:

- `engine/src/lib.rs`
- `engine/src/output.rs`
- `wasm/src/lib.rs`

Pontos relevantes observados:

- `ReplState` e `repl_run` ficam centralizados em `engine/src/lib.rs`
- a captura de saida fica em `engine/src/output.rs`
- a fronteira WASM atual e pequena, com `repl_new`, `repl_run`, `repl_complete`
- a fork de RustPython implementa `sys.settrace`, eventos `call/line/return/exception`, `current_frame()` e atributos de frame

## Escopo aprovado para a primeira versao

### Inclui

- trace por linha para o arquivo principal do usuario
- navegacao de timeline apos a execucao
- exibicao de stack frames do usuario
- exibicao de variaveis locais
- exibicao de memoria em formato simplificado, no estilo Python Tutor
- acumulacao de `stdout/stderr` por passo
- erro claro quando houver import de outro arquivo do usuario

### Nao inclui

- suporte a multiplos arquivos
- entrar em imports de arquivos do usuario
- entrar em modulos `spython.*`, stdlib embutida, builtins ou runtime interno
- tracing opcode a opcode
- retrocesso real do interpretador
- suporte inicial a programas interativos longos, `world`, animacao ou loops infinitos sem limite de passos

## Restricoes funcionais

### Regra de arquivo unico

O modo passo a passo so suporta programas de um unico arquivo.

Aceitar:

- imports de modulos embutidos permitidos
- imports de `spython.*`
- imports de stdlib suportada e embutida

Rejeitar:

- qualquer import first-party do usuario
- qualquer import relativo que resolva para arquivo do projeto
- qualquer `from x import ...` que resolva para arquivo do usuario

Mensagem sugerida:

`O modo passo a passo suporta apenas programas de um unico arquivo. Remova imports de arquivos do usuario para usar este modo.`

### Regra de filtragem de trace

O trace deve registrar apenas frames cujo `filename` seja exatamente o arquivo principal do usuario, por exemplo `user.py`.

Ignorar:

- `spython.*`
- stdlib frozen
- builtins
- bootstrap do runtime
- doctest runner
- qualquer modulo importado

Na pratica, o filtro deve ser por `co_filename` do frame, nao por nome de funcao.

## Estrategia geral

Nao tentar "voltar no tempo" no interpretador vivo.

Em vez disso:

1. executar o programa uma vez
2. capturar snapshots imutaveis durante a execucao
3. entregar uma timeline pronta para a UI
4. permitir avancar e retroceder sobre os snapshots

Esse modelo atende o requisito de navegacao sem exigir reversao real de estado.

## Arquitetura proposta

### Novo modulo no engine

Criar:

- `engine/src/trace.rs`

Responsabilidades:

- definir tipos de timeline
- validar restricao de arquivo unico
- instalar tracer
- executar o programa
- capturar snapshots
- serializar resultado para JSON ou estrutura exportavel

### Alteracoes em `engine/src/lib.rs`

Adicionar:

- `pub mod trace;`
- funcao publica como:

```rust
pub fn trace_source(
    source: &str,
    level: Level,
    max_steps: usize,
) -> Result<ExecutionTrace, TraceRunError>
```

### Alteracoes em `engine/src/output.rs`

Reaproveitar a captura existente.

Provavelmente adicionar helpers para extrair deltas sem copiar toda a saida a cada passo, por exemplo:

```rust
pub fn take_stdout_since(mark: &mut usize) -> String
pub fn take_stderr_since(mark: &mut usize) -> String
```

### Alteracoes em `wasm/src/lib.rs`

Adicionar uma export nova, por exemplo:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trace_source(
    code_ptr: *mut u8,
    code_len: usize,
    config_ptr: *mut u8,
    config_len: usize,
) -> *mut std::ffi::c_char
```

Config sugerida:

- `level=<n>`
- `max_steps=<n>`

## Modelo de dados sugerido

```rust
pub struct ExecutionTrace {
    pub steps: Vec<TraceStep>,
    pub truncated: bool,
    pub exit_kind: ExitKind,
    pub error: Option<TraceErrorInfo>,
}

pub struct TraceStep {
    pub step_index: u32,
    pub event: TraceEventKind,
    pub location: SourceLocation,
    pub stdout_delta: String,
    pub stderr_delta: String,
    pub stack: Vec<StackFrameView>,
    pub heap: HeapSnapshot,
}

pub enum TraceEventKind {
    Call,
    Line,
    Return,
    Exception,
    Finished,
}

pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

pub struct StackFrameView {
    pub frame_id: u64,
    pub name: String,
    pub line: u32,
    pub locals: Vec<BindingView>,
}

pub struct BindingView {
    pub name: String,
    pub value: ValueRef,
}

pub enum ValueRef {
    Primitive(PrimitiveValue),
    ObjectRef(u64),
}

pub enum PrimitiveValue {
    None,
    Bool(bool),
    Int(String),
    Float(f64),
    Str(String),
}

pub struct HeapSnapshot {
    pub objects: Vec<HeapObjectView>,
}

pub struct HeapObjectView {
    pub id: u64,
    pub kind: String,
    pub repr: String,
    pub attributes: Vec<BindingView>,
    pub elements: Vec<ValueRef>,
    pub entries: Vec<(ValueRef, ValueRef)>,
}
```

## Implementacao detalhada

### 1. Validacao de imports

Reaproveitar a logica ja existente de coleta de imports first-party em `engine/src/lib.rs`.

Implementar algo como:

```rust
fn ensure_trace_supported(db: &ProjectDatabase, user_file: File) -> Result<(), TraceRunError>
```

Se houver qualquer arquivo first-party do usuario importado, falhar antes da execucao.

### 2. Instalacao do tracer

A fork de RustPython ja oferece:

- `sys.settrace`
- eventos `call`, `line`, `return`, `exception`
- `current_frame()`
- `f_locals`, `f_globals`, `f_back`, `f_lineno`

Estrutura recomendada:

- registrar um modulo Rust auxiliar, por exemplo `_spython_trace`
- expor uma funcao Rust como `record_event(frame, event, arg)`
- instalar um tracer Python pequeno que delega para essa funcao Rust

Exemplo conceitual:

```python
import sys
from _spython_trace import record_event

def __spython_trace(frame, event, arg):
    return record_event(frame, event, arg)

sys.settrace(__spython_trace)
```

### 3. Filtro de frames

Criar funcao central:

```rust
fn is_user_frame(frame: &FrameRef, vm: &VirtualMachine) -> bool
```

Regra:

- incluir apenas `frame.f_code.co_filename == "user.py"`

Isso vale tanto para o frame atual quanto para a construcao da stack.

### 4. Captura de snapshots

Cada snapshot deve ser gerado em eventos:

- `call`
- `line`
- `return`
- `exception`
- `finished`

Estrutura auxiliar:

```rust
struct TraceCollector {
    steps: Vec<TraceStep>,
    stdout_mark: usize,
    stderr_mark: usize,
    max_steps: usize,
    truncated: bool,
}
```

Funcao central:

```rust
fn snapshot_current_state(
    vm: &VirtualMachine,
    frame: FrameRef,
    event: TraceEventKind,
    collector: &mut TraceCollector,
) -> PyResult<()>
```

### 5. Stack frames

Funcao sugerida:

```rust
fn build_stack_view(vm: &VirtualMachine, frame: FrameRef) -> PyResult<Vec<StackFrameView>>
```

Regras:

- subir por `f_back`
- incluir so frames de `user.py`
- cada frame mostra nome da funcao, linha atual e variaveis locais

### 6. Heap/memoria

Criar serializador proprio.

Funcao sugerida:

```rust
fn serialize_value(
    vm: &VirtualMachine,
    value: PyObjectRef,
    heap: &mut HeapBuilder,
) -> PyResult<ValueRef>
```

Suporte inicial:

- `None`
- `bool`
- `int`
- `float`
- `str`
- `list`
- `tuple`
- `dict`
- `set`
- instancias com `__dict__`
- dataclasses tratadas como objeto comum

Fallback:

- `repr`
- `kind`

Requisitos:

- ids estaveis por snapshot
- deteccao de ciclos
- preservacao de aliasing dentro do snapshot

### 7. Saida

Para cada passo, salvar:

- `stdout_delta`
- `stderr_delta`

Nao salvar a saida completa repetida em todos os passos.

### 8. Limite de passos

Adicionar `max_steps`.

Comportamento:

- ao atingir o limite, parar o tracing
- marcar `truncated = true`
- retornar `exit_kind = step_limit`

Essencial para loops longos ou infinitos.

## Backlog por fases

### Fase 1: backend minimo

- criar `engine/src/trace.rs`
- definir tipos de timeline
- validar `single-file only`
- instalar tracer
- filtrar frames para `user.py`
- capturar `line/call/return/exception`
- capturar stack e locals
- capturar `stdout/stderr`
- exportar resultado via WASM

### Fase 2: heap didatico

- serializar `list`, `tuple`, `dict`, `set`
- serializar objetos com `__dict__`
- ids estaveis por objeto
- deteccao de ciclos
- fallback para `repr`

### Fase 3: frontend

- timeline
- destaque de linha
- pilha de execucao
- painel de memoria
- painel de saida
- mensagens de truncamento e import nao suportado

## Ordem recomendada de desenvolvimento

1. criar tipos em `engine/src/trace.rs`
2. implementar validacao de arquivo unico
3. implementar deltas de saida em `engine/src/output.rs`
4. registrar modulo `_spython_trace`
5. instalar `sys.settrace`
6. capturar eventos e snapshots basicos
7. serializar stack e locals
8. exportar JSON via `wasm/src/lib.rs`
9. adicionar testes WASM
10. expandir serializacao de heap
11. integrar frontend

## Testes recomendados

### Rust

- filtro de frame por `filename`
- rejeicao de imports first-party
- serializacao de valores simples
- deteccao de ciclos

### WASM

Arquivo sugerido:

- `wasm/tests/trace_test.ts`

Casos minimos:

- expressao simples gera timeline
- chamada de funcao do usuario aparece na pilha
- `print()` aparece em `stdout_delta`
- `spython.image` nao aparece na stack
- import local falha com `unsupported_import`
- erro de runtime gera timeline parcial
- limite de passos marca `truncated=true`

## Criterios de aceite

- programa simples sem imports gera timeline navegavel
- avancar e retroceder nao reexecuta o programa
- stack mostra apenas frames do usuario
- modulos pre-definidos nao aparecem na timeline
- memoria mostra variaveis e objetos principais
- `print()` aparece incrementalmente
- erro de runtime ainda produz timeline ate a falha
- import de outro arquivo do usuario falha com mensagem especifica

## Riscos principais

- overhead de tracing por linha
- tamanho da timeline em loops grandes
- serializacao de heap ser a parte mais sensivel
- programas interativos exigirem escopo separado
- frontend web nao estar neste repositorio, entao o contrato de integracao deve ser mantido simples

## Recomendacao final

Implementar primeiro um MVP pos-execucao:

- linha atual
- stack
- locals
- saida incremental
- heap simplificado
- filtro estrito para `user.py`
- bloqueio de imports de arquivos do usuario

So depois expandir para uma visualizacao de heap mais rica no estilo Python Tutor.
