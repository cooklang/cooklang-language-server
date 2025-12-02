# Cooklang VS Code Extension

Language support for [Cooklang](https://cooklang.org/) recipe files.

## Features

- Syntax highlighting
- Diagnostics (errors/warnings)
- Hover information for ingredients, cookware, and timers
- Autocompletion for ingredients, cookware, units
- Document symbols outline
- Semantic token highlighting

## Installation

### Development Setup

1. Build the language server:
   ```bash
   cd /path/to/cooklang-language-server
   cargo build --release
   ```

2. Install extension dependencies:
   ```bash
   cd editors/vscode
   npm install
   npm run compile
   ```

3. Add the language server to your PATH or configure `cooklang.serverPath`:
   ```bash
   # Option A: Add to PATH
   export PATH="$PATH:/path/to/cooklang-language-server/target/release"

   # Option B: Configure in VS Code settings.json
   {
     "cooklang.serverPath": "/path/to/cooklang-language-server/target/release/cooklang-lsp"
   }
   ```

4. Open VS Code in the extension directory and press F5 to launch a development instance.

### Quick Test (without npm)

If you just want to test quickly:

1. Build the server: `cargo build --release`
2. Install the [Generic LSP Client](https://marketplace.visualstudio.com/items?itemName=llvm-vs-code-extensions.vscode-clangd) or similar
3. Configure it to run `cooklang-lsp` for `.cook` files

## Configuration

| Setting | Description | Default |
|---------|-------------|---------|
| `cooklang.serverPath` | Path to cooklang-lsp executable | (searches PATH) |
