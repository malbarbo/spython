The web playground lets you use spython directly in the browser, with no installation required.


# Layout

The interface is split into two panels:

- **Editor panel** (left or top): where you write Python code
- **REPL panel** (right or bottom): where output is displayed

The initial layout is chosen automatically based on screen dimensions: horizontal for wide screens and vertical for tall screens.


# Toolbar

- **Run** (▶): Formats and runs the definitions
- **Stop** (■): Interrupts execution
- **Theme** (☀): Toggles between light and dark themes
- **Layout**: Toggles between horizontal and vertical layout
- **Level**: Selects the teaching level (0-5)


# Keyboard shortcuts

| Shortcut | Description |
|----------|-------------|
| `Ctrl+r` | Run definitions |
| `Ctrl+f` | Format code |
| `Ctrl+j` | Focus on editor panel |
| `Ctrl+k` | Focus on REPL panel |
| `Ctrl+d` | Show/hide editor panel |
| `Ctrl+i` | Show/hide REPL panel |
| `Ctrl+l` | Toggle horizontal/vertical layout |
| `Ctrl+t` | Toggle light/dark theme |
| `Ctrl+?` | Show help dialog |
| `Esc` | Close help dialog |


# How to use

1. Write your definitions in the editor panel
2. Press `Ctrl+r` or click **Run**
3. Use the REPL to evaluate expressions using the definitions

The **Run** button (or `Ctrl+r`) formats the code, checks types and annotations, runs doctests, and loads definitions into the REPL. After that, you can call the functions defined in the editor directly in the REPL.

The REPL checks types and annotations on each input. Definitions without annotations or with constructs forbidden for the selected level are rejected before execution.


# Teaching levels

The level selector in the toolbar controls which Python constructs are allowed:

| Level | Name       | Added constructs                                                      |
|-------|------------|-----------------------------------------------------------------------|
| 0     | Functions  | `def`, `return`, scalars, `str` indexing                              |
| 1     | Selection  | `if`/`elif`/`else`                                                    |
| 2     | Types      | `class` (Enum / `@dataclass`), `match`                                |
| 3     | Repetition | `list` literals, `for`, `while`, `+=`                                 |
| 4     | Classes    | `class` with methods, `dict`/`set`, comprehensions, `lambda`          |
| 5     | Full       | unrestricted (only annotations are still required)                    |


# Themes

The playground supports two themes based on the Zed editor:

- **One Light** -- light theme (default)
- **One Dark** -- dark theme

The preference is saved in the browser.
