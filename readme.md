# Papyrus

Inspired:

- [russo-2025/papyrus-compiler](https://github.com/russo-2025/papyrus-compiler)
- [matklad's resilient LL parser](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)

## Crates

| Crate            | Role                                                                                              |
| ---------------- | ------------------------------------------------------------------------------------------------- |
| `papyrus_intern` | Global, case-insensitive string interner (`Symbol`, `Name`).                                      |
| `papyrus_lexer`  | Zero-copy lexer (`Lexer<'a>`). Newlines are tokens; `\` + newline is a line continuation.         |
| `papyrus_parser` | Resilient, line-oriented LL parser producing a flat lossless CST viewed through `SyntaxNode<'a>`. |
| `papyrus_ide`    | Workspace index, name resolution, type inference and IDE features, independent of LSP.            |
| `papyrus_lsp`    | Language server over stdio.                                                                       |

Every script of `tests/fixtures/papyrus-compiler-test-sources` must parse without errors and
without semantic diagnostics (`cargo test`).

## Language server

Features: diagnostics, hover, go to definition, references, document highlight, completion
(members, locals, overridable events), signature help, document/workspace symbols, folding,
semantic tokens.

Custom requests:

- `papyrus/restart`: soft restart. Drops all indexes, applies `{ settings }`, re-indexes and
  republishes diagnostics while keeping open documents.
- `papyrus/status`: indexed files, open files, indexing state.
- `papyrus/syntaxTree`: debug dump of a document's syntax tree.

Build the VS Code extension with `npm run release`, then launch `Run Papyrus LSP Extension`.
Set `papyrus.importDirectories` to the game's `Data/Source/Scripts` for full resolution.

## Todo

- [x] Tokenizer
- [x] Concrete Syntax Tree
- [x] Concrete Syntax Tree Parser
- [ ] Formatter
- [x] Semantic tree (index + name resolution)
- [ ] Type checker
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
