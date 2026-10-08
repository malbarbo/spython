# Plano: Comando Stepper — TUI fullscreen para execução passo a passo

## Contexto

O spython já tem um sistema de tracing (`engine/src/trace.rs`) que executa um programa Python uma vez e captura snapshots em cada evento (line, call, return, exception). O resultado é um `ExecutionTrace` com steps, stack frames e locals. Atualmente, isso é exposto apenas como JSON via WASM. O objetivo é adicionar uma visualização TUI fullscreen no CLI que permite navegar a timeline com setas e ver código, pilha, heap e saída.

## Estilo de visualização: Python Tutor

A visualização deve seguir o estilo do Python Tutor (pythontutor.com):

- **Pilha (stack)**: mostra os frames de execução com variáveis locais. Variáveis de tipos primitivos (int, float, bool, str, None) mostram o valor inline. Variáveis de tipos compostos (list, dict, tuple, set, objetos) mostram uma referência (seta/ponteiro) para o heap.
- **Heap**: mostra os objetos compostos com seus conteúdos. Aliasing fica explícito — duas variáveis que referenciam o mesmo objeto apontam para o mesmo bloco no heap.
- **Código**: à esquerda, com a linha atual destacada.
- **Saída**: stdout acumulado.

Isso requer implementar a serialização de heap (Phase 2 do plano original de trace) como parte do stepper, não depois.

## Modelo de dados necessário para heap

O trace atual tem:
```rust
pub struct StackFrameView {
    pub name: String,
    pub line: u32,
    pub locals: Vec<(String, String)>,  // nome → repr()
}
```

Para o estilo Python Tutor, precisa evoluir para:
```rust
pub struct StackFrameView {
    pub name: String,
    pub line: u32,
    pub locals: Vec<BindingView>,
}

pub struct BindingView {
    pub name: String,
    pub value: ValueRef,
}

pub enum ValueRef {
    Primitive(String),    // int, float, bool, str, None — mostrado inline
    HeapRef(u64),         // referência a objeto no heap
}

pub struct HeapSnapshot {
    pub objects: Vec<HeapObject>,
}

pub struct HeapObject {
    pub id: u64,
    pub kind: String,         // "list", "dict", "tuple", "set", "object"
    pub elements: Vec<ValueRef>,          // para list, tuple, set
    pub entries: Vec<(ValueRef, ValueRef)>, // para dict
    pub attrs: Vec<BindingView>,          // para objetos com __dict__
    pub repr: String,                     // fallback
}
```

Cada `TraceStep` deve incluir `heap: HeapSnapshot` com os objetos alocados.

## Layout da TUI

```
┌─ Código ─────────────────────┬─ Pilha ──────────┬─ Heap ────────────┐
│   1  x: list[int] = [1, 2]  │ ┌─ <module> ───┐ │ id0: list         │
│   2  y: list[int] = x       │ │ x ──────────────→ [1, 2]           │
│ ▸ 3  y.append(3)            │ │ y ──────────────→                   │
│   4  print(x)               │ └───────────────┘ │                   │
│                              │                   │                   │
├─ Saída ──────────────────────┴───────────────────┴───────────────────┤
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ Passo 3/6  linha 3 [line]  ← → navegar  q sair                      │
└──────────────────────────────────────────────────────────────────────┘
```

O heap fica à direita da pilha. Referências de variáveis a objetos no heap podem ser indicadas com setas (`──→`) ou com IDs (`→ #0`).

Alternativa simplificada (sem setas reais, usando IDs):

```
┌─ Código ─────────────────────┬─ Pilha ──────────┬─ Heap ────────────┐
│   1  x: list[int] = [1, 2]  │ ┌─ <module> ───┐ │ #0 list           │
│   2  y: list[int] = x       │ │ x → #0       │ │ ┌───┬───┬───┐     │
│ ▸ 3  y.append(3)            │ │ y → #0       │ │ │ 1 │ 2 │ 3 │     │
│   4  print(x)               │ └───────────────┘ │ └───┴───┴───┘     │
├─ Saída ──────────────────────┴───────────────────┴───────────────────┤
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

## Abordagem

Adicionar um subcomando `stepper` ao CLI e um comando `:stepper` ao REPL. Usar `crossterm` para controle do terminal (raw mode, alternate screen, eventos de teclado). Renderização manual com caracteres UTF-8 box drawing.

## Arquivos a modificar/criar

### Modificar

1. **`engine/src/trace.rs`** — evoluir tipos para suportar heap (ValueRef, HeapSnapshot, HeapObject), implementar serialização de heap no tracer Python, tornar Deserialize incondicional
2. **`cli/Cargo.toml`** — adicionar `crossterm`, `serde_json`, `serde`
3. **`cli/main.rs`** — adicionar variante `Stepper` ao enum `Command`, `mod stepper`, e handler
4. **`cli/repl.rs`** — adicionar comando `:stepper file.py` e atualizar `:help`

### Criar

5. **`cli/stepper/mod.rs`** — `StepperApp`, event loop, `run_stepper()` entry point
6. **`cli/stepper/render.rs`** — funções de desenho para cada painel (código, pilha, heap, saída, status)

## Implementação detalhada

### 1. Engine: Evolução do modelo de dados (`engine/src/trace.rs`)

Adicionar `BindingView`, `ValueRef`, `HeapSnapshot`, `HeapObject` aos tipos exportados. Modificar `StackFrameView` para usar `Vec<BindingView>` em vez de `Vec<(String, String)>`.

O tracer Python precisa:
- Identificar primitivos (int, float, bool, str, None) → inline
- Para compostos (list, dict, tuple, set, objetos com `__dict__`), atribuir um ID estável (usando `id()` do Python) e registrar no heap
- Detectar ciclos (visited set)
- Preservar aliasing (mesmo `id()` = mesmo objeto no heap)

Funções nativas adicionais:
- `_push_binding(name, is_primitive, value_or_id)` — substitui `_push_local`
- `_push_heap_object(id, kind, repr)` — registra objeto no heap
- `_push_heap_element(id, is_primitive, value_or_id)` — elemento de list/tuple/set
- `_push_heap_entry(id, key_is_prim, key, val_is_prim, val)` — entrada de dict
- `_push_heap_attr(id, name, is_primitive, value_or_id)` — atributo de objeto

### 2. Dependências (`cli/Cargo.toml`)

```toml
crossterm = "0.28"
serde_json = "1"
serde = { version = "1", features = ["derive"] }
```

### 3. Comando CLI (`cli/main.rs`)

Adicionar variante `Stepper { level: u8, file: PathBuf }` ao enum `Command` (bpaf).
Handler: ler arquivo → `trace_source` → deserializar → `stepper::run_stepper`.

### 4. Comando REPL (`cli/repl.rs`)

`:stepper file.py` — mesma lógica do handler CLI.

### 5. Módulo stepper (`cli/stepper/mod.rs`)

```rust
pub struct StepperApp {
    source_lines: Vec<String>,
    trace: ExecutionTrace,
    current_step: usize,
    stdout_at: Vec<String>,    // precomputado
    term_size: (u16, u16),
    source_scroll: usize,
}
```

Event loop: `Right`/`Left` avança/retrocede, `Home`/`End` início/fim, `q`/`Esc` sai.
Cleanup do terminal garantido com `catch_unwind`.

### 6. Renderização (`cli/stepper/render.rs`)

Layout com 3 painéis superiores (código, pilha, heap) + saída + status.
Caracteres UTF-8: `┌ ─ ┐ │ └ ┘ ├ ┤ ┬ ┴ ▸`
Cores via crossterm: linha atual com fundo invertido.

## Serialização de heap: detalhes

O tracer Python deve:
1. Para cada variável, verificar `type(v).__name__`:
   - `int`, `float`, `bool`, `str`, `NoneType` → primitivo, valor inline
   - `list`, `tuple`, `dict`, `set` → heap ref, serializar conteúdo
   - Outros com `__dict__` → heap ref, serializar atributos
2. Usar `id(v)` como ID do objeto. Manter um set de IDs já visitados para evitar ciclos.
3. Elementos de listas/dicts podem ser primitivos ou refs a outros objetos no heap.
4. Limitar profundidade de recursão (ex: 3 níveis) para evitar explosion.

## Sequência de implementação

1. Evoluir tipos em `engine/src/trace.rs` (modelo com heap)
2. Atualizar tracer Python para serializar heap
3. Atualizar testes Rust e WASM
4. Adicionar dependências ao CLI
5. Criar `cli/stepper/mod.rs` (event loop)
6. Criar `cli/stepper/render.rs` (rendering)
7. Integrar no `cli/main.rs` e `cli/repl.rs`
8. Testes manuais
9. `make check` + `cargo test` + `make test-wasm`

## Verificação

1. `cargo build` compila
2. `make check` limpo
3. `cargo test --workspace` passa
4. `make test-wasm` passa
5. `cargo run -p cli -- stepper file.py` mostra TUI com código, pilha, heap
6. Aliasing visível (ex: `x` e `y` apontam para o mesmo `#id` no heap)
7. Navegação com setas funciona, q sai, terminal restaurado
8. `:stepper file.py` no REPL funciona e volta ao REPL
