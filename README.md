# Cooklang Language Server

A [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) (LSP) implementation for [Cooklang](https://cooklang.org/), the markup language for recipes.

This language server provides rich editor support for `.cook` files, including syntax highlighting, auto-completion, and real-time error checking. It works with any LSP-compatible editor like VS Code, Neovim, Emacs, and Helix.

## Features

- **🔍 Diagnostics** - Real-time syntax checking and validation with helpful error messages
- **✨ Auto-completion** - Smart suggestions for ingredients (`@`), cookware (`#`), timers (`~`), and units (`%`)
- **🎨 Syntax Highlighting** - Semantic token-based colorization for all Cooklang elements
- **📖 Hover Information** - View ingredient details, quantities, and notes on hover
- **📑 Document Outline** - Navigate recipe structure with sections, ingredients, and cookware

## Installation

The Cooklang Language Server is available through the [`cook`](https://github.com/cooklang/cookcli) command-line tool.

### Using with cook

The language server can be invoked via:

```bash
cook lsp
```

This starts the LSP server and communicates over stdin/stdout, which is the standard LSP transport mechanism.

## Editor Setup

Once you have [`cook`](https://github.com/cooklang/cookcli) installed, configure your editor to use it as the language server for `.cook` files.

### VS Code

Install the [Cooklang extension](https://github.com/cooklang/CookVSCode) from the marketplace or GitHub.

### Neovim

Using `nvim-lspconfig`:

```lua
require'lspconfig'.cooklang.setup{
  cmd = { "cook", "lsp" },
  filetypes = { "cook" },
  root_dir = require'lspconfig'.util.root_pattern(".git"),
}
```

### Helix

Add to your `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "cooklang"
scope = "source.cook"
file-types = ["cook"]
language-servers = ["cooklang-lsp"]

[language-server.cooklang-lsp]
command = "cook"
args = ["lsp"]
```

### Other Editors

Any LSP-compatible editor can use this language server. Configure it to run `cook lsp` for `.cook` files.

## Supported Features

### Diagnostics

Get real-time feedback on syntax errors and warnings:

- Missing closing braces in ingredients, cookware, or timers
- Invalid quantity formats
- Malformed metadata
- Extension-specific validation

### Auto-completion

Context-aware suggestions triggered by:

- `@` - Suggests ingredients from the current recipe and workspace
- `#` - Suggests cookware (both used in recipe and common items like "pot", "pan", "oven")
- `~` - Suggests time units (seconds, minutes, hours)
- `%` - Suggests measurement units (g, kg, ml, cups, tbsp, etc.)

### Syntax Highlighting

Semantic token-based highlighting for:

- Ingredients (`@potato{2}`)
- Cookware (`#pot`)
- Timers (`~{25%minutes}`)
- Quantities and units
- Comments (`--` and `[- -]`)
- Metadata keys and values
- Section headers (`== Preparation ==`)

### Additional Features

- **Hover Information** - View ingredient quantities, notes, and modifiers
- **Document Symbols** - Navigate recipe structure via outline view
- **Go to Definition** - Jump to first use of ingredients
- **Find References** - Find all uses of an ingredient

## Technology

This language server is built with:

- **Rust** - For performance and reliability
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** - LSP framework
- **[cooklang-rs](https://github.com/cooklang/cooklang-rs)** - Official Cooklang parser

## Development

This repository contains the core LSP implementation. For building from source or contributing:

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# The binary will be at target/release/cooklang-lsp
```

## License

Licensed under the [MIT License](./LICENSE).

## Resources

- [Cooklang Specification](https://cooklang.org/docs/spec/)
- [Cooklang Website](https://cooklang.org/)
- [`cook` CLI Tool](https://github.com/cooklang/cookcli)
- [Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
