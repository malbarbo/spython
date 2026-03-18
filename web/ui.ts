import { ansiToHtml } from "./ansi.ts";

declare const Prism: {
    highlight(code: string, grammar: unknown, language: string): string;
    languages: Record<string, unknown>;
};
import {
    KEYDOWN,
    KEYPRESS,
    KEYUP,
    UIChannel,
    WorkerMessage,
} from "./ui_channel.ts";

declare class CodeFlask {
    constructor(
        el: HTMLElement,
        options: {
            language: string;
            lineNumbers: boolean;
            tabSize?: number;
            handleNewLineIndentation?: boolean;
        },
    );
    updateCode(code: string): void;
    onUpdate(callback: (code: string) => void): void;
    getCode(): string;
}

type AppState =
    | { kind: "loading"; progress: number }
    | { kind: "error"; message: string }
    | {
        kind: "ready";
        running: boolean;
        level: number;
        layout: "horizontal" | "vertical";
        view: "split" | "editor" | "repl";
        splitSize: number;
        helpVisible: boolean;
        resizing: boolean;
        dirty: boolean;
        history: string[];
        historyIndex: number;
    };

class App {
    private state: AppState = { kind: "loading", progress: 0 };
    private runAfterFormat = false;
    private replInput: HTMLDivElement | null = null;
    private replPrompt: HTMLDivElement | null = null;
    private lastSvg: HTMLDivElement | null = null;
    private lastActive: HTMLElement | null = null;

    private readonly flask: CodeFlask;
    private readonly channel: UIChannel;
    private readonly main: HTMLElement;
    private readonly loading: HTMLElement;
    private readonly runButton: HTMLButtonElement;
    private readonly stopButton: HTMLButtonElement;
    private readonly resizeHandle: HTMLElement;
    private readonly editorPanel: HTMLElement;
    private readonly replPanel: HTMLElement;
    private readonly helpOverlay: HTMLElement;
    private readonly help: HTMLElement;
    private readonly levelSelect: HTMLSelectElement;
    private readonly themeToggle: HTMLButtonElement;
    private readonly layoutHorizontal: HTMLButtonElement;
    private readonly layoutVertical: HTMLButtonElement;

    private readonly shortcuts = new Map<string, () => void>([
        ["ctrl+?", () => this.showHelp()],
        ["ctrl+j", () => this.focusEditor()],
        ["ctrl+k", () => this.focusRepl()],
        ["ctrl+r", () => this.formatThenRun()],
        ["ctrl+f", () => this.format()],
        ["ctrl+d", () => this.toggleEditor()],
        ["ctrl+i", () => this.toggleRepl()],
        ["ctrl+l", () => this.toggleLayout()],
    ]);

    constructor() {
        this.main = document.getElementById("main")!;
        this.loading = document.getElementById("loading")!;
        this.runButton = document.getElementById(
            "run-button",
        )! as HTMLButtonElement;
        this.stopButton = document.getElementById(
            "stop-button",
        )! as HTMLButtonElement;
        this.resizeHandle = document.getElementById("resize-handle")!;
        this.editorPanel = document.getElementById("editor-panel")!;
        this.replPanel = document.getElementById("repl-panel")!;
        this.helpOverlay = document.getElementById("help-overlay")!;
        this.help = document.getElementById("help")!;
        this.levelSelect = document.getElementById(
            "level-select",
        )! as HTMLSelectElement;
        this.themeToggle = document.getElementById(
            "theme-toggle",
        )! as HTMLButtonElement;
        const savedTheme = localStorage.getItem("spython-theme") ?? "light";
        document.documentElement.setAttribute("data-theme", savedTheme);
        this.layoutHorizontal = document.getElementById(
            "layout-horizontal",
        )! as HTMLButtonElement;
        this.layoutVertical = document.getElementById(
            "layout-vertical",
        )! as HTMLButtonElement;

        this.flask = new CodeFlask(this.editorPanel, {
            language: "python",
            lineNumbers: true,
            tabSize: 4,
            handleNewLineIndentation: false,
        });
        this.flask.updateCode(
            document.getElementById("default-code")?.textContent ?? "",
        );
        this.setupEditorAutoIndent();
        this.flask.onUpdate(() => {
            if (
                this.state.kind === "ready" && !this.state.running &&
                !this.state.dirty
            ) {
                this.state.dirty = true;
                this.render();
            }
        });

        const worker = new Worker("worker.js", { type: "module" });
        worker.onmessage = (e: MessageEvent<WorkerMessage>) =>
            this.onWorkerMessage(e);

        this.channel = new UIChannel(worker);

        this.setupListeners();
        this.render();
    }

    private setupEditorAutoIndent(): void {
        const textarea = this.editorPanel.querySelector<HTMLTextAreaElement>(
            "textarea.codeflask__textarea",
        );
        if (!textarea) return;

        const TAB = 4;
        const INDENT = " ".repeat(TAB);
        const DEDENT_KEYWORDS = /^\s*(return|pass|break|continue)\b/;

        const updateCode = (newCode: string, newPos: number) => {
            this.flask.updateCode(newCode);
            requestAnimationFrame(() => {
                textarea.selectionStart = newPos;
                textarea.selectionEnd = newPos;
            });
        };

        textarea.addEventListener("keydown", (e: KeyboardEvent) => {
            const code = textarea.value;
            const pos = textarea.selectionStart;
            const before = code.slice(0, pos);
            const lineStart = before.lastIndexOf("\n") + 1;
            const line = before.slice(lineStart);

            if (e.key === "Enter") {
                const match = line.match(/^(\s*)/);
                const currentIndent = match ? match[1] : "";

                let newIndent = currentIndent;
                if (line.trimEnd().endsWith(":")) {
                    newIndent = currentIndent + INDENT;
                } else if (DEDENT_KEYWORDS.test(line)) {
                    if (currentIndent.length >= INDENT.length) {
                        newIndent = currentIndent.slice(INDENT.length);
                    }
                }

                e.preventDefault();
                const insertion = "\n" + newIndent;
                updateCode(
                    before + insertion + code.slice(pos),
                    pos + insertion.length,
                );
            } else if (e.key === "Backspace") {
                // If only spaces before cursor on this line, snap to previous indent level
                if (
                    textarea.selectionStart === textarea.selectionEnd &&
                    line.length > 0 &&
                    line.trim() === ""
                ) {
                    const spaces = line.length;
                    const remove = spaces % TAB === 0 ? TAB : spaces % TAB;
                    e.preventDefault();
                    updateCode(
                        code.slice(0, pos - remove) + code.slice(pos),
                        pos - remove,
                    );
                }
            }
        });
    }

    private setupListeners(): void {
        this.runButton.addEventListener("click", () => this.formatThenRun());
        this.stopButton.addEventListener("click", () => this.stop());
        this.layoutHorizontal.addEventListener(
            "click",
            () => this.setLayout("horizontal"),
        );
        this.layoutVertical.addEventListener(
            "click",
            () => this.setLayout("vertical"),
        );
        const updateLevel = () => {
            if (this.state.kind === "ready") {
                this.state.level = parseInt(this.levelSelect.value);
            }
        };
        this.levelSelect.addEventListener("change", updateLevel);
        this.levelSelect.addEventListener("input", updateLevel);
        this.themeToggle.addEventListener("click", () => this.toggleTheme());
        this.replPanel.addEventListener("click", () => this.onReplPanelClick());
        this.resizeHandle.addEventListener(
            "mousedown",
            (e) => this.startResize(e),
        );
        document.addEventListener("mousemove", (e) => this.resize(e));
        document.addEventListener("mouseup", () => this.stopResize());
        this.resizeHandle.addEventListener(
            "touchstart",
            (e) => this.startResize(e),
        );
        document.addEventListener("touchmove", (e) => this.resize(e));
        document.addEventListener("touchend", () => this.stopResize());
        document.addEventListener("keydown", (e) => this.onKeyDown(e));
    }

    private onWorkerMessage(event: MessageEvent<WorkerMessage>): void {
        const data = event.data;
        switch (data.cmd) {
            case "error":
                this.state = { kind: "error", message: data.data };
                this.render();
                break;
            case "progress":
                this.state = { kind: "loading", progress: data.data };
                this.render();
                break;
            case "ready": {
                const prev = this.state;
                this.state = prev.kind === "ready"
                    ? {
                        ...prev,
                        running: false,
                        dirty: data.hadErrors ? prev.dirty : false,
                    }
                    : {
                        kind: "ready",
                        running: false,
                        level: 0,
                        layout: window.innerWidth >= window.innerHeight
                            ? "horizontal"
                            : "vertical",
                        view: "split",
                        splitSize: 50,
                        helpVisible: false,
                        resizing: false,
                        dirty: true,
                        history: [],
                        historyIndex: -1,
                    };
                this.render();
                this.lastSvg = null;
                if (prev.kind !== "ready") {
                    this.replPanel.replaceChildren();
                }
                this.channel.setBuffer(data.buffer);
                this.addInputLine();
                break;
            }
            case "formatted":
                this.flask.updateCode(data.data);
                if (this.runAfterFormat) {
                    this.runAfterFormat = false;
                    this.run();
                }
                break;
            case "write":
                this.addOutput(data.fd, data.data);
                break;
            case "svg":
                this.addSvg(data.data);
                break;
            case "input":
                this.addInputPrompt();
                break;
        }
    }

    private onKeyDown(event: KeyboardEvent): void {
        if (event.key === "Escape") {
            event.preventDefault();
            if (this.state.kind === "ready") this.hideHelp();
            return;
        }
        if (this.state.kind !== "ready" || this.state.helpVisible) return;
        const combo = `${event.ctrlKey ? "ctrl+" : ""}${event.key}`;
        const action = this.shortcuts.get(combo);
        if (action) {
            event.preventDefault();
            action();
        }
    }

    private onReplPanelClick(): void {
        if (window.getSelection()?.toString().length !== 0) return;
        this.focusRepl();
    }

    private render(): void {
        const s = this.state;
        if (s.kind === "loading") {
            this.loading.style.display = "block";
            this.loading.textContent = `Loading ${Math.round(s.progress)}%`;
        } else if (s.kind === "error") {
            this.loading.style.display = "block";
            this.loading.textContent = s.message;
        } else {
            this.loading.style.display = "none";
        }

        this.runButton.disabled = s.kind !== "ready" || s.running;
        this.stopButton.disabled = s.kind !== "ready" || !s.running;
        this.levelSelect.disabled = s.kind !== "ready" || s.running;

        if (s.kind !== "ready") return;

        this.levelSelect.value = String(s.level);

        if (this.replPrompt) {
            this.replPrompt.textContent = s.dirty ? "\u25CF >" : ">";
        }

        this.main.style.flexDirection = s.layout === "horizontal"
            ? "row"
            : "column";
        this.layoutHorizontal.disabled = s.layout === "horizontal";
        this.layoutVertical.disabled = s.layout === "vertical";
        this.resizeHandle.style.cursor = s.layout === "horizontal"
            ? "col-resize"
            : "row-resize";
        this.resizeHandle.style.width = s.layout === "horizontal"
            ? "8px"
            : "100%";
        this.resizeHandle.style.height = s.layout === "horizontal"
            ? "100%"
            : "8px";

        this.editorPanel.style.display = s.view !== "repl" ? "flex" : "none";
        this.replPanel.style.display = s.view !== "editor" ? "flex" : "none";
        this.resizeHandle.style.display = s.view === "split"
            ? "initial"
            : "none";

        this.helpOverlay.style.display = s.helpVisible ? "block" : "none";
        this.help.style.display = s.helpVisible ? "block" : "none";

        if (s.resizing) {
            document.body.style.cursor = s.layout === "horizontal"
                ? "col-resize"
                : "row-resize";
        } else {
            document.body.style.cursor = "";
        }

        if (s.layout === "horizontal") {
            this.editorPanel.style.height = "100%";
            this.editorPanel.style.width = `${s.splitSize}%`;
        } else if (s.view !== "editor") {
            this.editorPanel.style.width = "100%";
            this.editorPanel.style.height = `${s.splitSize}%`;
        } else {
            this.editorPanel.style.width = "100%";
            this.editorPanel.style.height = "100%";
        }
    }

    private postLoad(): void {
        if (this.state.kind !== "ready") return;
        this.state.level = parseInt(this.levelSelect.value);
        this.state.running = true;
        this.render();
        this.channel.load(this.flask.getCode(), this.state.level);
    }

    private postRun(code: string): void {
        if (this.state.kind !== "ready") return;
        this.state.running = true;
        this.render();
        this.channel.run(code);
    }

    private focusEditor(): void {
        const s = this.state;
        if (s.kind !== "ready") return;
        const input = this.editorPanel.querySelector<HTMLTextAreaElement>(
            "textarea:not([disabled])",
        );
        if (s.view !== "repl" && input) input.focus();
    }

    private focusRepl(): void {
        const s = this.state;
        if (s.kind !== "ready") return;
        if (s.view !== "editor" && this.replInput) {
            const ta = this.replInput.querySelector("textarea");
            if (ta) ta.focus();
            else this.replInput.focus();
        }
    }

    private showHelp(): void {
        if (this.state.kind !== "ready") return;
        this.state.helpVisible = true;
        this.render();
        if (document.activeElement instanceof HTMLElement) {
            this.lastActive = document.activeElement;
            this.lastActive.blur();
        } else {
            this.lastActive = null;
        }
    }

    private toggleTheme(): void {
        const current = document.documentElement.getAttribute("data-theme") ??
            "light";
        const next = current === "light" ? "dark" : "light";
        document.documentElement.setAttribute("data-theme", next);
        localStorage.setItem("spython-theme", next);
    }

    private hideHelp(): void {
        if (this.state.kind !== "ready") return;
        this.state.helpVisible = false;
        this.render();
        this.lastActive?.focus();
    }

    private run(): void {
        const s = this.state;
        if (s.kind === "ready" && !s.running) {
            this.replInput = null;
            this.replPanel.replaceChildren();
            this.postLoad();
        }
    }

    private formatThenRun(): void {
        if (this.state.kind !== "ready") return;
        this.runAfterFormat = true;
        this.format();
    }

    private stop(): void {
        const s = this.state;
        if (s.kind === "ready" && s.running) {
            s.running = false;
            this.render();
            this.channel.stop();
        }
    }

    private format(): void {
        this.channel.format(this.flask.getCode());
    }

    private toggleEditor(): void {
        if (this.state.kind !== "ready") return;
        const hiding = this.state.view !== "repl";
        this.state.view = hiding ? "repl" : "split";
        this.render();
        if (hiding) this.focusRepl();
    }

    private toggleRepl(): void {
        if (this.state.kind !== "ready") return;
        const hiding = this.state.view !== "editor";
        this.state.view = hiding ? "editor" : "split";
        this.render();
        if (hiding) this.focusEditor();
    }

    private toggleLayout(): void {
        if (this.state.kind !== "ready") return;
        this.setLayout(
            this.state.layout === "horizontal" ? "vertical" : "horizontal",
        );
    }

    private setLayout(layout: "horizontal" | "vertical"): void {
        if (this.state.kind !== "ready") return;
        this.state.layout = layout;
        this.render();
    }

    private startResize(e: Event): void {
        e.preventDefault();
        if (this.state.kind !== "ready") return;
        this.state.resizing = true;
        this.render();
    }

    private resize(e: Event): void {
        const s = this.state;
        if (s.kind !== "ready" || !s.resizing) return;

        let clientX: number;
        let clientY: number;

        if (typeof TouchEvent !== "undefined" && e instanceof TouchEvent) {
            clientX = e.touches[0].clientX;
            clientY = e.touches[0].clientY;
        } else {
            clientX = (e as MouseEvent).clientX;
            clientY = (e as MouseEvent).clientY;
        }

        let newSize: number;
        if (s.layout === "horizontal") {
            newSize = (clientX / this.main.clientWidth) * 100;
        } else {
            newSize = ((clientY - this.main.getBoundingClientRect().top) /
                this.main.clientHeight) *
                100;
        }

        if (newSize > 20 && newSize < 80) {
            s.splitSize = newSize;
            this.render();
        }
    }

    private stopResize(): void {
        if (this.state.kind !== "ready" || !this.state.resizing) return;
        this.state.resizing = false;
        this.render();
    }

    private highlightPython(code: string): string {
        if (Prism.languages.python) {
            return Prism.highlight(code, Prism.languages.python, "python");
        }
        return code;
    }

    private addInputLine(): void {
        const inputContainer = document.createElement("div");
        inputContainer.className = "repl-input-container";

        const dirty = this.state.kind === "ready" && this.state.dirty;
        const prompt = (this.replPrompt = document.createElement("div"));
        prompt.className = "repl-prompt";
        prompt.textContent = dirty ? "\u25CF >" : ">";

        const wrapper = document.createElement("div");
        wrapper.className = "repl-input-wrapper";

        const highlight = document.createElement("pre");
        highlight.className = "repl-input-highlight";

        const textarea = document.createElement("textarea");
        textarea.className = "repl-input-edit";
        textarea.rows = 1;
        textarea.spellcheck = false;

        wrapper.appendChild(highlight);
        wrapper.appendChild(textarea);
        inputContainer.appendChild(prompt);
        inputContainer.appendChild(wrapper);
        this.replPanel.appendChild(inputContainer);

        // Store textarea reference for focusRepl()
        this.replInput = wrapper as unknown as HTMLDivElement;

        textarea.focus();

        const syncHighlight = () => {
            highlight.innerHTML = this.highlightPython(textarea.value) ||
                "\n";
            // Auto-resize textarea to match content
            textarea.style.height = "auto";
            textarea.style.height = textarea.scrollHeight + "px";
        };
        syncHighlight();

        textarea.addEventListener("input", syncHighlight);

        let savedInput = "";

        textarea.addEventListener("keydown", (e: KeyboardEvent) => {
            if (this.state.kind !== "ready") return;
            const s = this.state;

            if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                const code = textarea.value.trim();
                if (code) {
                    s.history.push(code);
                    s.historyIndex = -1;
                    textarea.disabled = true;
                    textarea.style.display = "none";
                    highlight.innerHTML = this.highlightPython(code);
                    this.postRun(code);
                }
            } else if (e.key === "ArrowUp") {
                e.preventDefault();
                if (s.history.length === 0) return;
                if (s.historyIndex === -1) {
                    savedInput = textarea.value;
                    s.historyIndex = s.history.length - 1;
                } else if (s.historyIndex > 0) {
                    s.historyIndex--;
                }
                textarea.value = s.history[s.historyIndex];
                syncHighlight();
            } else if (e.key === "ArrowDown") {
                e.preventDefault();
                if (s.historyIndex === -1) return;
                if (s.historyIndex < s.history.length - 1) {
                    s.historyIndex++;
                    textarea.value = s.history[s.historyIndex];
                } else {
                    s.historyIndex = -1;
                    textarea.value = savedInput;
                }
                syncHighlight();
            }
        });
    }

    private addInputPrompt(): void {
        const input = document.createElement("span");
        input.className = "repl-input";
        input.style.color = "var(--prompt)";
        input.contentEditable = "true";
        input.spellcheck = false;

        // Append to the last output line if it exists (e.g., input("Name: "))
        const lastChild = this.replPanel.lastElementChild;
        if (lastChild && lastChild.classList.contains("repl-line")) {
            lastChild.appendChild(input);
        } else {
            const container = document.createElement("div");
            container.className = "repl-input-container";
            container.appendChild(input);
            this.replPanel.appendChild(container);
        }
        input.focus();
        this.replPanel.scrollTop = this.replPanel.scrollHeight;

        input.addEventListener("keydown", (e: KeyboardEvent) => {
            if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                const text = input.textContent ?? "";
                input.contentEditable = "false";
                this.channel.submitInput(text);
            }
        });
    }

    private addOutput(_fd: number, text: string): void {
        const html = ansiToHtml(text);
        const output = document.createElement("div");
        output.className = "repl-line";
        output.innerHTML = html;
        this.replPanel.appendChild(output);
        this.replPanel.scrollTop = this.replPanel.scrollHeight;
    }

    private addSvg(svg: string): void {
        if (!this.lastSvg) {
            this.lastSvg = document.createElement("div");
            this.lastSvg.style.fontSize = "0";
            this.lastSvg.style.outline = "none";
            this.replPanel.appendChild(this.lastSvg);
            this.lastSvg.tabIndex = 0;
            const handler = (type: number) => (event: KeyboardEvent) => {
                this.channel.enqueueKeyEvent({
                    type,
                    key: event.key,
                    alt: event.altKey,
                    ctrl: event.ctrlKey,
                    shift: event.shiftKey,
                    meta: event.metaKey,
                    repeat: event.repeat,
                });
            };
            this.lastSvg.addEventListener("keypress", handler(KEYPRESS));
            this.lastSvg.addEventListener("keydown", handler(KEYDOWN));
            this.lastSvg.addEventListener("keyup", handler(KEYUP));
            this.lastSvg.focus();
            this.replPanel.scrollTop = this.replPanel.scrollHeight;
        }
        this.lastSvg.innerHTML = svg;
    }
}

document.addEventListener("DOMContentLoaded", () => new App());
