// Build script: inlines CodeFlask, PrismJS Python, ui.js (with worker.js
// embedded) into index.html to produce a single dist/index.html file.
//
// Usage: deno run --allow-read --allow-write web/inline.ts

const [
    htmlPath,
    uiJsPath,
    workerJsPath,
    codeflaskPath,
    prismPythonPath,
    outputPath,
] = [
    "web/spython.html",
    "build/ui.js",
    "build/worker.js",
    "build/codeflask.min.js",
    "build/prism-python.min.js",
    "dist/index.html",
];

let html = await Deno.readTextFile(htmlPath);
let uiJs = await Deno.readTextFile(uiJsPath);
const workerJs = await Deno.readTextFile(workerJsPath);
const codeflaskJs = await Deno.readTextFile(codeflaskPath);
const prismPythonJs = await Deno.readTextFile(prismPythonPath);

// Embed worker.js as a blob URL inside ui.js.
// The main thread computes the absolute wasm URL (since blob workers can't
// resolve relative paths) and patches it into the worker code before creating
// the blob.
const workerLiteral = "`" + workerJs.replaceAll("\\", "\\\\").replaceAll(
    "`",
    "\\`",
).replaceAll("$", "\\$") + "`";

const workerReplacement = [
    `const __wasmUrl = new URL("engine.wasm", location.href).href;`,
    `const __workerCode = ${workerLiteral}.replace('"engine.wasm"', JSON.stringify(__wasmUrl));`,
    `const worker = new Worker(URL.createObjectURL(new Blob([__workerCode], { type: "application/javascript" })));`,
].join("\n    ");

uiJs = uiJs.replace(
    `const worker = new Worker("worker.js", {\n      type: "module"\n    });`,
    workerReplacement,
);

// Replace CodeFlask CDN script with inline
html = html.replace(
    /    <script src="https:\/\/unpkg\.com\/codeflask\/build\/codeflask\.min\.js"><\/script>/,
    `    <script>${codeflaskJs}</script>`,
);

// Replace PrismJS Python CDN script with inline
html = html.replace(
    /    <script src="https:\/\/unpkg\.com\/prismjs\/components\/prism-python\.min\.js"><\/script>/,
    `    <script>${prismPythonJs}</script>`,
);

// Replace ui.js module script with inline
html = html.replace(
    /    <script type="module" src="spython\.js"><\/script>/,
    `    <script type="module">${uiJs}</script>`,
);

await Deno.writeTextFile(outputPath, html);
console.log(`Written ${outputPath}`);
