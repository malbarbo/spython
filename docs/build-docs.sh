#!/bin/bash
set -e

mkdir -p dist/docs

build() {
  local output=$1; shift
  pandoc "$@" \
    --standalone --toc \
    --highlight-style=docs/one-light.theme \
    --css=docs/style.css \
    --embed-resources \
    --include-after-body=docs/theme-toggle.html \
    --metadata author="Marco A L Barbosa" \
    --metadata date="2026" \
    -o "$output"
  echo "Generated $output"
}

# Portuguese
build dist/docs/cli.pt-br.html \
  docs/pt-br/index-cli.md docs/pt-br/cli.md \
  --metadata title="Guia do Spython - CLI"

build dist/docs/web.pt-br.html \
  docs/pt-br/index-web.md docs/pt-br/web.md \
  --metadata title="Guia do Spython - Web"

# English
build dist/docs/cli.en.html \
  docs/en/index-cli.md docs/en/cli.md \
  --metadata title="Spython Guide - CLI"

build dist/docs/web.en.html \
  docs/en/index-web.md docs/en/web.md \
  --metadata title="Spython Guide - Web"
