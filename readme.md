# Papyrus

Inspired:

- [russo-2025/papyrus-compiler](https://github.com/russo-2025/papyrus-compiler)
- [matklad's resilient LL parser](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)

## Crates

| Crate              | Role                                                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------- |
| `papyrus_intern`   | Global, case-insensitive string interner (`Symbol`, `Name`).                                      |
| `papyrus_lexer`    | Zero-copy lexer (`Lexer<'a>`). Newlines are tokens; `\` + newline is a line continuation.         |
| `papyrus_parser`   | Resilient, line-oriented LL parser producing a flat lossless CST viewed through `SyntaxNode<'a>`. |
| `papyrus_ide`      | Workspace index, name resolution, type inference and IDE features, independent of LSP.            |
| `papyrus_analyzer` | Language server over stdio.                                                                       |

Every script of `tests/fixtures/papyrus-compiler-test-sources` must parse without errors and
without semantic diagnostics (`cargo test`).

## Language server

Features: diagnostics (syntax, names, types), hover (with keyword documentation and examples),
go to definition, references, document highlight, completion (members, locals, overridable events,
flags), signature help, document/workspace symbols, folding, semantic tokens, code lenses and
formatting.

Custom requests:

- `papyrus/restart`: soft restart. Drops all indexes, applies `{ settings }`, re-indexes and
  republishes diagnostics while keeping open documents.
- `papyrus/status`: indexed files, open files, indexing state.
- `papyrus/syntaxTree`: debug dump of a document's syntax tree.

- `papyrus/compileCommands`: the compiler invocations described by `papyrus.toml` (run by the client).

Build the VS Code extension with `npm run release`, then launch `Run Papyrus Analyzer Extension`.

### `papyrus.toml`

Optional. Without it the server indexes the workspace folders (plus `papyrus.importDirectories`)
and picks the first `TESV_Papyrus_Flags.flg` it finds. With it, the imports and the flags file come
from the manifest, and the compiler can be run from:

- the `▶ Compile` code lens above `ScriptName`;
- the `Compile…` status bar button, which compiles the scripts picked in a list (remembered);
- the explorer context menu on one or several `.psc` files, the editor title button or `Ctrl+Shift+B`.

`papyrus.toml` is searched in the workspace folders, their parents and up to three levels of
subdirectories. `Papyrus: Create papyrus.toml` writes a template.

`papyrus.toml` is committed and shared, so machine specific paths are written as `${NAME}`
variables. A variable is taken from, first wins:

1. the `papyrus.var` editor setting (VS Code user `settings.json`), or `papyrus_analyzer --var NAME=VALUE`
   / `init_options = { var = { ... } }` for other editors;
2. `[var]` of `papyrus.local.toml` next to the manifest (not committed, added to `.gitignore`);
3. `[var]` of `papyrus.toml` (shared defaults).

An undefined variable is an error (the status bar turns red). Like in VS Code settings,
`${env:NAME}` reads the environment variable `NAME` and `${workspaceFolder}` (or
`${workspaceFolder:name}`) is a workspace folder. `${game:SkyrimSE}` is the installation directory
found in the registry (`SkyrimLE`, `SkyrimSE`, `SkyrimVR`, `Enderal`, `EnderalSE`); the registry
only exists on Windows, so elsewhere set `SKYRIM_DIR` to a path instead. Values of
variables may use them too, e.g. `"papyrus.var": { "SKYRIM_DIR": "${game:SkyrimSE}" }`. Hover a `${...}` or a `[var]` key in `papyrus.toml` to see
its value and where it comes from.

```toml
[package]
name = "MyMod"

[compiler]
path = "${SKYRIM_DIR}/Papyrus Compiler/PapyrusCompiler.exe"
args = []

[build]
sources = ["."]                             # scripts to compile, recursively
output = "output"                           # .pex output, keeps the directories of sources
imports = ["${SKYRIM_DIR}/Data/Scripts/Source"]
flags = "${SKYRIM_DIR}/Data/Scripts/Source/TESV_Papyrus_Flags.flg"
optimize = true
release = false
final = false
quiet = true
```

```jsonc
// VS Code user settings.json
"papyrus.var": { "SKYRIM_DIR": "${game:SkyrimSE}" }  // or an absolute path
```

```sh
# Other editors: pass variables on the command line (see `papyrus_analyzer --help`).
papyrus_analyzer --var "SKYRIM_DIR=D:/Steam/steamapps/common/Skyrim Special Edition"
```

Without `papyrus.toml`, the game scripts below `SKYRIM_DIR` (or the installation found in the registry)
are imported automatically; add more
directories with `papyrus.importDirectories` (variables allowed, e.g. `${SKYRIM_DIR}/Data/Scripts/Source`).

### Formatter

`papyrus_analyzer fmt [--check] [PATH]...` formats scripts from the command line, and the editor
formats through the language server. Options come from the editor, then `.editorconfig`
(`indent_style`, `indent_size`, `end_of_line`, `insert_final_newline`), then `[format]` of
`papyrus.toml`:

```toml
[format]
indent-style = "space"        # space or tab
indent-size = 4
keyword-case = "canonical"    # canonical, preserve, PascalCase, camelCase, lowercase, UPPERCASE
max-blank-lines = 1
newline = "auto"              # auto, lf, crlf
final-newline = true

[format.keywords]               # single keywords, winning over keyword-case
as = "As"
```

Scripts with syntax errors are not formatted, and the result is checked to contain the same tokens.

### Packaging

`node dist.js` copies the server next to the extension; `node dist.js --vsix --target <triple>`
also packages `papyrus-analyzer-<platform>.vsix`. The CI build and release workflows use the same
command (`npm run package` locally).

## Todo

- [x] Tokenizer
- [x] Concrete Syntax Tree
- [x] Concrete Syntax Tree Parser
- [x] Formatter
- [x] Semantic tree (index + name resolution)
- [x] Type checker
- [ ] Linter
- [x] Language server
- [ ] Compiler
- [ ] Papyrus Virtual Machine(For unit test)

## License

This code was created by studying [rust-analyzer](https://github.com/rust-lang/rust-analyzer).
In other words, this code as a whole can be considered a product of rust-analyzer.

```txt
SPDX-FileCopyrightText: (C) 2026 rust-analyzer team
SPDX-License-Identifier: Apache-2.0 OR MIT
```
