# Plano: Desenho do Heap para Execucao Passo a Passo

## Objetivo

Adicionar uma visualizacao do heap para o stepper com estas propriedades:

- amigavel para iniciantes
- estavel entre passos
- fiel a aliasing e identidade dos objetos
- capaz de mostrar estruturas misturadas
- reutilizavel tanto no CLI quanto na web

O requisito central e este:

- quando o usuario avanca ou retrocede um passo, o desenho nao deve "dancar"
- objetos existentes devem manter sua posicao enquanto continuarem vivos
- o desenho deve apenas crescer, encolher ou revelar partes novas do heap

## Contexto atual

O trace atual ja fornece a base necessaria:

- cada valor referenciado na stack aponta para `Ref { object_id }`
- cada `TraceStep` ja contem `heap: Vec<HeapObject>`
- os `object_id` agora sao estaveis ao longo de toda a execucao
- a UI atual do stepper ja sabe resolver `Ref` para exibir valores inline ou com `#id`

Portanto, o trabalho principal agora nao esta no engine. Ele esta no stepper:

- converter `TraceStep` em um grafo intermediario
- reconhecer padroes estruturais
- calcular posicoes estaveis
- renderizar esse layout no CLI e, depois, em SVG

## Conclusoes da avaliacao

### 1. Nao usar um unico algoritmo generico como base do produto

Um algoritmo generico de layout de grafo nao e a melhor base para esta feature.

Motivos:

- a visualizacao e pedagogica, nao apenas geometrica
- estruturas comuns como `list`, `dict`, cadeia e arvore precisam aparecer de forma previsivel
- o requisito de estabilidade entre passos e mais importante que a compactacao perfeita
- o CLI precisa de um subconjunto simples, algo que bibliotecas genericas nao resolvem bem

### 2. Usar um layout hibrido

O desenho deve combinar algoritmos especializados:

- `list`: layout linear
- `dict`: layout tabular
- cadeia linear: layout em linha com setas
- arvore: layout hierarquico estavel
- grafo geral: fallback separado

### 3. Classificacao por forma, nao por nome

O sistema nao deve assumir nomes como `next`, `left` ou `right`.

Deve usar:

- tipo bruto do trace
- cardinalidade das referencias
- ordem estrutural dos slots
- fan-in e fan-out
- presenca de ciclos
- compartilhamento de nos

Isso permite lidar com estruturas misturadas, por exemplo:

- `list` em que cada item e uma lista encadeada
- `dict` apontando para arvores
- objetos contendo `list` e `dict`

Heuristicas mais agressivas, como reconhecer lista encadeada codificada com `tuple` no estilo do Python Tutor, podem vir depois.

### 4. Estabilidade exige estado persistente

Nao devemos recalcular o layout inteiro a cada passo.

Precisamos de um `LayoutState` persistente por `object_id`, com:

- posicao
- dimensoes
- ordem local entre irmaos
- metadados do grupo estrutural

Regra principal:

- nos antigos nao mudam de lugar
- nos novos ganham espaco novo
- nos removidos desaparecem
- a compactacao nao pode recentralizar o desenho inteiro

## O que foi observado sobre bibliotecas e referencias externas

### Python Tutor

O Python Tutor mostra uma visualizacao claramente pedagogica, com opcoes como:

- inline primitives
- render all objects on the heap
- modos diferentes para ponteiros

Isso sugere um renderer especializado, nao um unico layout generico.

Nao foi encontrada uma descricao publica clara do algoritmo exato de layout usado internamente. Essa parte permanece como inferencia a partir da UI e da documentacao publica.

### ELK

O Eclipse Layout Kernel e a melhor referencia externa para fallback de grafos gerais na web:

- tem algoritmo em camadas
- tem opcoes de ordem do modelo
- tem modo `interactive`, voltado a minimizar mudancas no layout

Mesmo assim, ele nao deve ser a base do produto, porque a feature precisa de layouts pedagogicos e de uma politica propria de estabilidade.

### Dagre e Graphviz

- `Dagre` e util para DAGs, mas nao atende sozinho o requisito de estabilidade e pedagogia
- `Graphviz` e bom para gerar layout estatico, mas nao e adequado como nucleo de uma visualizacao passo a passo estavel

## Decisao arquitetural

Implementar o nucleo de layout aqui no projeto.

Uso recomendado:

- layout proprio para `list`, `dict`, cadeia e arvore
- fallback proprio simples para grafo geral no CLI
- opcao futura de integrar `ELK.js` apenas na web, somente para componentes classificados como grafo geral

## Modelo intermediario proposto

Criar um modelo intermediario no stepper, derivado de `TraceStep`:

```rust
struct HeapGraph {
    roots: Vec<RootRef>,
    nodes: BTreeMap<u64, GraphNode>,
    edges: Vec<GraphEdge>,
}

struct RootRef {
    frame_index: usize,
    local_name: String,
    target: u64,
}

struct GraphNode {
    object_id: u64,
    kind: NodeKind,
    slots: Vec<NodeSlot>,
    label: String,
    schema: NodeSchema,
}

struct NodeSlot {
    slot_id: SlotId,
    label: String,
    role: SlotRole,
    value: SlotValue,
}

enum SlotValue {
    InlineScalar(String),
    Ref(u64),
}
```

Esse modelo tem tres objetivos:

- normalizar o trace cru para o layout
- preservar ordem estrutural dos slots
- permitir que imutaveis continuem no heap do trace, mas aparecam inline na UI

## Classificacao estrutural

### Principio

A classificacao e local e composicional.

Ela nao responde:

- "o heap inteiro e uma arvore?"

Ela responde:

- "este subgrafo se comporta como cadeia?"
- "este container referencia uma arvore?"
- "este grupo precisa cair no fallback de grafo geral?"

### Shapes iniciais

#### 1. Linear container

Aplicado a:

- `list`

Renderizacao:

- caixas em linha
- ordem fixa por indice

#### 2. Map container

Aplicado a:

- `dict`

Renderizacao:

- linhas de pares `key/value`
- ordem do trace preservada

#### 3. Chain

Reconhecer subgrafos que:

- tenham schema recorrente
- tenham no maximo um sucessor estrutural do mesmo schema
- nao tenham ramificacao estrutural relevante
- nao tenham ciclo
- tenham fan-in estrutural no maximo 1

Isso cobre listas encadeadas baseadas em objetos ou dataclasses sem depender de nomes de campos.

#### 4. Tree

Reconhecer subgrafos que:

- sejam aciclicos
- tenham fan-in estrutural no maximo 1
- tenham filhos ordenados
- nao compartilhem nos entre pais

#### 5. Generic graph

Fallback para casos com:

- alias forte
- ciclos
- multiplos pais
- mistura ambigua de estruturas

## Composicao de layouts

Estruturas podem estar misturadas.

Exemplo:

- uma `list` cujos itens sao listas encadeadas

O layout precisa ser recursivo:

1. desenhar o container principal
2. para cada slot com referencia, classificar o subgrafo alcancavel a partir dali
3. anexar o sublayout adequado ao slot correspondente

Isso permite compor:

- `list` de `list`
- `list` de cadeias
- `dict` para arvores
- objetos com campos para containers e subestruturas

## Politica de estabilidade

Criar um `LayoutState` persistente, separado do trace:

```rust
struct LayoutState {
    node_boxes: HashMap<u64, Rect>,
    first_seen_step: HashMap<u64, usize>,
    group_id_by_node: HashMap<u64, GroupId>,
    sibling_order_keys: HashMap<(u64, SlotId), usize>,
}
```

Regras:

- se um `object_id` ja tem caixa atribuida, a caixa permanece
- um novo objeto ocupa espaco novo, sem empurrar o heap inteiro
- a ordem entre irmaos deve ser estavel
- a remocao de objetos nao deve recentralizar tudo

Prioridade:

1. estabilidade
2. legibilidade
3. compacidade

## Algoritmos de layout recomendados

### 1. List

Implementar no projeto.

Politica:

- celulas em linha
- crescimento para a direita
- itens antigos nao se movem

### 2. Dict

Implementar no projeto.

Politica:

- pares em coluna
- novas entradas entram no fim
- entradas antigas nao se movem

### 3. Chain

Implementar no projeto.

Politica:

- nos em linha horizontal
- crescimento para a direita
- sem relayout dos nos anteriores

### 4. Tree

Implementar no projeto, mas nao usar Reingold-Tilford puro na primeira versao.

Motivo:

- Reingold-Tilford e excelente para desenho estatico
- mas tende a recentralizar subarvores quando elas crescem
- isso conflita com o requisito principal de estabilidade

Politica recomendada para v1:

- layout top-down
- ordem dos filhos pela ordem dos slots
- cada ramo ocupa uma faixa reservada
- crescimento da arvore abre espaco apenas na regiao afetada

Depois, se necessario, podemos estudar compactacao local para SVG.

### 5. Generic graph

Primeira versao:

- fallback incremental proprio
- camadas por distancia a partir das raizes
- ordem estavel por primeira aparicao e ordem de porta
- sem relayout global

Versao futura para web:

- avaliar integracao opcional com `ELK.js`
- usar apenas para componentes classificados como grafo geral
- manter politica local de ancoragem e posicoes persistentes

## CLI e Web

### CLI

Primeira versao do desenho no CLI:

- suportar bem `list`
- suportar bem `dict`
- suportar bem cadeia
- suportar arvore pequena
- usar fallback textual para grafo geral

Nao tentar resolver grafo denso com arte ASCII complexa.

### Web / SVG

A web e o melhor alvo para a visualizacao completa:

- caixas
- portas
- arestas ortogonais
- destaque de alias
- melhor composicao de sublayouts
- melhor espaco para animacao leve

Recomendacao:

- implementar primeiro um renderer SVG sobre o mesmo nucleo de layout
- deixar o CLI consumir uma projecao simplificada dessa geometria

## Estrutura de modulos sugerida

Criar no `cli/stepper`:

- `heap_graph.rs`
  - converte `TraceStep` em `HeapGraph`
- `layout/mod.rs`
  - pipeline de layout
- `layout/state.rs`
  - `LayoutState`
- `layout/classify.rs`
  - classificadores estruturais
- `layout/place.rs`
  - algoritmos de posicionamento incremental
- `layout/svg.rs`
  - renderer SVG
- `layout/ascii.rs`
  - renderer simplificado para CLI

## Sequencia de implementacao

### Fase 1: Infraestrutura

1. Criar `HeapGraph`
2. Criar testes de conversao a partir de `TraceStep`
3. Criar `LayoutState`
4. Definir tipos geometricos comuns (`Point`, `Rect`, `EdgePath`)

### Fase 2: Shapes basicos

5. Implementar layout de `list`
6. Implementar layout de `dict`
7. Implementar composicao simples de containers com filhos referenciados

### Fase 3: Reconhecimento estrutural

8. Implementar classificador de `chain`
9. Implementar layout incremental de `chain`
10. Implementar classificador de `tree`
11. Implementar layout estavel de `tree`

### Fase 4: Fallback geral

12. Implementar fallback de grafo geral
13. Garantir que alias, ciclos e compartilhamento nao quebrem o layout

### Fase 5: Renderers

14. Implementar renderer SVG
15. Integrar SVG com o caminho web
16. Implementar renderer simplificado para CLI

### Fase 6: Refinamentos

17. Melhorar roteamento visual de arestas
18. Sincronizar selecao stack/heap
19. Avaliar integracao opcional com `ELK.js` para fallback web
20. Estudar heuristicas adicionais, como listas encadeadas codificadas com `tuple`

## Casos de teste obrigatorios

### Estabilidade

- `list` crescendo por `append`
- `dict` recebendo novas entradas
- cadeia crescendo na cauda
- arvore crescendo dos dois lados
- remocao de referencias sem mover nos sobreviventes

### Estruturas misturadas

- `list` de `list`
- `list` de cadeias
- `dict` para arvores
- objeto com `list` e `dict`

### Corretude estrutural

- alias entre dois locals
- alias entre dois containers
- ciclo simples
- multiplos pais no mesmo subgrafo

### UX

- layout pequeno no CLI continua legivel
- fallback textual aparece para casos que o CLI nao consegue desenhar bem
- SVG nao muda a posicao de nos antigos quando o passo avanca

## Riscos principais

### 1. Tentar compactar demais

Layouts mais compactos costumam mover nos antigos. Isso vai contra o objetivo principal.

### 2. Classificacao agressiva demais

Se o classificador assumir demais sobre a forma, pode desenhar errado e confundir o usuario.

### 3. Misturar layout e renderer cedo demais

O nucleo de layout precisa ser independente do CLI e do SVG.

## Recomendacao final

Seguir com esta estrategia:

- nucleo de layout implementado no projeto
- classificacao por forma estrutural local
- composicao recursiva de sublayouts
- estabilidade guiada por `object_id`
- SVG como alvo principal do desenho completo
- CLI como projecao simplificada
- `ELK.js` apenas como opcao futura para o fallback web de grafo geral

## Referencias externas

- Python Tutor: <https://pythontutor.com/visualize.html/visualize.html>
- Python Tutor repository: <https://github.com/pathrise-eng/pathrise-python-tutor>
- Reingold-Tilford: <https://www.reingold.co/tidier-drawings.pdf>
- ELK getting started: <https://eclipse.dev/elk/gettingstarted.html>
- ELK layered: <https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html>
- ELK interactive: <https://eclipse.dev/elk/reference/options/org-eclipse-elk-interactive.html>
- ELK model order: <https://eclipse.dev/elk/reference/options/org-eclipse-elk-layered-considerModelOrder-strategy.html>
- ELK force node model order: <https://eclipse.dev/elk/reference/options/org-eclipse-elk-layered-crossingMinimization-forceNodeModelOrder.html>
- Dagre repository: <https://github.com/dagrejs/dagre>
- Dagre package: <https://www.npmjs.com/package/dagre>
- graphviz-rust: <https://docs.rs/graphviz-rust>
