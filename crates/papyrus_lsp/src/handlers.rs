//! Request handlers: protocol in, [`papyrus_ide`] query, protocol out.

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
    CompletionTextEdit, DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams,
    DocumentSymbolParams, DocumentSymbolResponse, Documentation, FoldingRange, FoldingRangeKind,
    FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, InsertTextFormat, Location, LocationLink, MarkupContent, MarkupKind,
    ParameterInformation, ParameterLabel, ReferenceParams, SemanticToken, SemanticTokens,
    SemanticTokensParams, SemanticTokensRangeParams, SemanticTokensRangeResult,
    SemanticTokensResult, SignatureHelp, SignatureHelpParams, SignatureInformation,
    SymbolInformation, TextDocumentPositionParams, TextEdit, WorkspaceSymbolParams,
    WorkspaceSymbolResponse,
};
use papyrus_ide::{FileId, FoldKind, TextRange, TextSize};
use rustc_hash::FxHashMap;

use crate::{conv::Lines, ext, server::Server};

/// Maximum number of workspace symbols returned.
const MAX_WORKSPACE_SYMBOLS: usize = 256;

impl Server {
    fn position(&self, pos: &TextDocumentPositionParams) -> Option<(FileId, Lines, TextSize)> {
        let file = self.file(&pos.text_document.uri).ok()?;
        let lines = Lines::of(&self.ws, file)?;
        let offset = lines.offset(pos.position);
        Some((file, lines, offset))
    }

    fn location(
        &self,
        file: FileId,
        range: TextRange,
        cache: &mut FxHashMap<FileId, Lines>,
    ) -> Option<Location> {
        let lines = match cache.entry(file) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(Lines::of(&self.ws, file)?),
        };
        Some(Location {
            uri: crate::conv::path_to_uri(&self.ws.file(file).path),
            range: lines.range(range),
        })
    }

    pub(crate) fn hover(&mut self, params: HoverParams) -> Option<Hover> {
        let (file, lines, offset) = self.position(&params.text_document_position_params)?;
        let hover = papyrus_ide::hover(&self.ws, file, offset)?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover.markdown,
            }),
            range: Some(lines.range(hover.range)),
        })
    }

    pub(crate) fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Option<GotoDefinitionResponse> {
        let (file, lines, offset) = self.position(&params.text_document_position_params)?;
        let targets = papyrus_ide::goto_definition(&self.ws, file, offset);
        if targets.is_empty() {
            return None;
        }
        let link_support = self
            .caps
            .text_document
            .as_ref()
            .and_then(|t| t.definition.as_ref())
            .and_then(|d| d.link_support)
            .unwrap_or(false);
        let mut cache = FxHashMap::default();
        cache.insert(file, lines);
        if link_support {
            let mut links = Vec::new();
            for t in targets {
                let full = self.location(t.file, t.full_range, &mut cache)?;
                let focus = self.location(t.file, t.focus_range, &mut cache)?;
                links.push(LocationLink {
                    origin_selection_range: None,
                    target_uri: full.uri,
                    target_range: full.range,
                    target_selection_range: focus.range,
                });
            }
            Some(GotoDefinitionResponse::Link(links))
        } else {
            let locations = targets
                .into_iter()
                .filter_map(|t| self.location(t.file, t.focus_range, &mut cache))
                .collect();
            Some(GotoDefinitionResponse::Array(locations))
        }
    }

    pub(crate) fn references(&mut self, params: ReferenceParams) -> Option<Vec<Location>> {
        let (file, lines, offset) = self.position(&params.text_document_position)?;
        let refs =
            papyrus_ide::references(&self.ws, file, offset, params.context.include_declaration);
        let mut cache = FxHashMap::default();
        cache.insert(file, lines);
        Some(refs.into_iter().filter_map(|(f, r)| self.location(f, r, &mut cache)).collect())
    }

    pub(crate) fn document_highlight(
        &mut self,
        params: DocumentHighlightParams,
    ) -> Option<Vec<DocumentHighlight>> {
        let (file, lines, offset) = self.position(&params.text_document_position_params)?;
        let highlights = papyrus_ide::highlight(&self.ws, file, offset);
        Some(
            highlights
                .into_iter()
                .map(|h| DocumentHighlight {
                    range: lines.range(h.range),
                    kind: Some(if h.write {
                        DocumentHighlightKind::WRITE
                    } else {
                        DocumentHighlightKind::READ
                    }),
                })
                .collect(),
        )
    }

    pub(crate) fn completion(&mut self, params: CompletionParams) -> Option<CompletionResponse> {
        let (file, lines, offset) = self.position(&params.text_document_position)?;
        let completions = papyrus_ide::completions(&self.ws, file, offset);
        let snippets = self
            .caps
            .text_document
            .as_ref()
            .and_then(|t| t.completion.as_ref())
            .and_then(|c| c.completion_item.as_ref())
            .and_then(|i| i.snippet_support)
            .unwrap_or(false);

        let items = completions
            .items
            .into_iter()
            .map(|item| {
                let (new_text, format) = match (&item.snippet, snippets) {
                    (Some(snippet), true) => (snippet.clone(), InsertTextFormat::SNIPPET),
                    _ => (plain_insert_text(&item), InsertTextFormat::PLAIN_TEXT),
                };
                CompletionItem {
                    label: item.label.clone(),
                    kind: Some(completion_kind(item.kind)),
                    detail: item.detail,
                    documentation: item.documentation.map(markdown),
                    sort_text: Some(item.sort_text),
                    filter_text: Some(
                        item.label.split('(').next().unwrap_or(&item.label).to_owned(),
                    ),
                    insert_text_format: Some(format),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: lines.range(item.replace_range),
                        new_text,
                    })),
                    data: item
                        .resolve
                        .map(|(f, i)| serde_json::json!({ "file": f.0, "member": i })),
                    ..Default::default()
                }
            })
            .collect();
        Some(CompletionResponse::List(CompletionList {
            is_incomplete: completions.is_incomplete,
            items,
        }))
    }

    pub(crate) fn resolve_completion(&mut self, mut item: CompletionItem) -> CompletionItem {
        let target = item.data.as_ref().and_then(|d| {
            let file = d.get("file")?.as_u64()? as u32;
            let member = d.get("member")?.as_u64()? as u32;
            Some((FileId(file), member))
        });
        if let Some((file, member)) = target {
            let known = self.ws.files().any(|(id, _)| id == file);
            if known && item.documentation.is_none() {
                item.documentation = papyrus_ide::member_docs(&self.ws, file, member).map(markdown);
            }
        }
        item
    }

    pub(crate) fn signature_help(&mut self, params: SignatureHelpParams) -> Option<SignatureHelp> {
        let (file, _, offset) = self.position(&params.text_document_position_params)?;
        let help = papyrus_ide::signature_help(&self.ws, file, offset)?;
        let label_utf16 = |byte: u32| help.label[..byte as usize].encode_utf16().count() as u32;
        let parameters = help
            .params
            .iter()
            .map(|&(start, end)| ParameterInformation {
                label: ParameterLabel::LabelOffsets([label_utf16(start), label_utf16(end)]),
                documentation: None,
            })
            .collect();
        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: help.label.clone(),
                documentation: help.documentation.map(markdown),
                parameters: Some(parameters),
                active_parameter: help.active_param,
            }],
            active_signature: Some(0),
            active_parameter: help.active_param,
        })
    }

    pub(crate) fn document_symbols(
        &mut self,
        params: DocumentSymbolParams,
    ) -> Option<DocumentSymbolResponse> {
        let file = self.file(&params.text_document.uri).ok()?;
        let lines = Lines::of(&self.ws, file)?;
        let symbols = papyrus_ide::document_symbols(&self.ws, file);
        Some(DocumentSymbolResponse::Nested(
            symbols.into_iter().map(|s| document_symbol(&lines, s)).collect(),
        ))
    }

    pub(crate) fn workspace_symbols(
        &mut self,
        params: WorkspaceSymbolParams,
    ) -> Option<WorkspaceSymbolResponse> {
        let symbols =
            papyrus_ide::workspace_symbols(&self.ws, &params.query, MAX_WORKSPACE_SYMBOLS);
        let mut cache = FxHashMap::default();
        let infos = symbols
            .into_iter()
            .filter_map(|s| {
                let location = self.location(s.file, s.range, &mut cache)?;
                #[allow(deprecated)]
                Some(SymbolInformation {
                    name: s.name,
                    kind: symbol_kind(s.kind),
                    tags: None,
                    deprecated: None,
                    location,
                    container_name: s.container,
                })
            })
            .collect();
        Some(WorkspaceSymbolResponse::Flat(infos))
    }

    pub(crate) fn folding_ranges(
        &mut self,
        params: FoldingRangeParams,
    ) -> Option<Vec<FoldingRange>> {
        let file = self.file(&params.text_document.uri).ok()?;
        let lines = Lines::of(&self.ws, file)?;
        let folds = papyrus_ide::folding_ranges(&self.ws, file);
        Some(
            folds
                .into_iter()
                .filter_map(|f| {
                    let start = lines.position(f.range.start()).line;
                    let end = lines.position(f.range.end()).line;
                    // For regions the closing keyword's line stays visible.
                    let end = if f.kind == FoldKind::Region { end.checked_sub(1)? } else { end };
                    (end > start).then(|| FoldingRange {
                        start_line: start,
                        end_line: end,
                        kind: Some(match f.kind {
                            FoldKind::Region => FoldingRangeKind::Region,
                            FoldKind::Comment => FoldingRangeKind::Comment,
                        }),
                        ..Default::default()
                    })
                })
                .collect(),
        )
    }

    pub(crate) fn semantic_tokens_full(
        &mut self,
        params: SemanticTokensParams,
    ) -> Option<SemanticTokensResult> {
        let file = self.file(&params.text_document.uri).ok()?;
        let data = self.encode_semantic_tokens(file, None)?;
        Some(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data }))
    }

    pub(crate) fn semantic_tokens_range(
        &mut self,
        params: SemanticTokensRangeParams,
    ) -> Option<SemanticTokensRangeResult> {
        let file = self.file(&params.text_document.uri).ok()?;
        let lines = Lines::of(&self.ws, file)?;
        let range = lines.text_range(params.range);
        let data = self.encode_semantic_tokens(file, Some(range))?;
        Some(SemanticTokensRangeResult::Tokens(SemanticTokens { result_id: None, data }))
    }

    fn encode_semantic_tokens(
        &self,
        file: FileId,
        range: Option<TextRange>,
    ) -> Option<Vec<SemanticToken>> {
        let lines = Lines::of(&self.ws, file)?;
        let tokens = papyrus_ide::semantic_tokens(&self.ws, file, range);
        let mut data = Vec::with_capacity(tokens.len());
        let (mut prev_line, mut prev_col) = (0u32, 0u32);
        for tok in tokens {
            let text = &lines.text[tok.range];
            // Tokens may not span lines: split multi-line comments.
            let mut offset = tok.range.start();
            for part in text.split_inclusive('\n') {
                let content = part.trim_end_matches(['\n', '\r']);
                let len = content.encode_utf16().count() as u32;
                if len > 0 {
                    let pos = lines.position(offset);
                    let delta_line = pos.line - prev_line;
                    let delta_start =
                        if delta_line == 0 { pos.character - prev_col } else { pos.character };
                    data.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: len,
                        token_type: tok.ty as u32,
                        token_modifiers_bitset: tok.modifiers,
                    });
                    prev_line = pos.line;
                    prev_col = pos.character;
                }
                offset += TextSize::of(part);
            }
        }
        Some(data)
    }

    pub(crate) fn syntax_tree(&mut self, params: ext::SyntaxTreeParams) -> String {
        let Ok(file) = self.file(&params.text_document.uri) else { return String::new() };
        let Some(text) = self.ws.text(file) else { return String::new() };
        papyrus_parser::parse(&text).debug_dump()
    }
}

/// Without snippet support, insert the bare name (and `()` for calls).
fn plain_insert_text(item: &papyrus_ide::CompletionItem) -> String {
    match &item.snippet {
        Some(snippet) if snippet.contains("$0\n") => item.label.clone(),
        Some(_) => format!("{}()", item.label),
        None => item.label.clone(),
    }
}

fn markdown(value: String) -> Documentation {
    Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value })
}

fn completion_kind(kind: papyrus_ide::CompletionKind) -> CompletionItemKind {
    use papyrus_ide::CompletionKind as K;
    match kind {
        K::Keyword => CompletionItemKind::KEYWORD,
        K::Function => CompletionItemKind::FUNCTION,
        K::Event => CompletionItemKind::EVENT,
        K::Property => CompletionItemKind::PROPERTY,
        K::Variable => CompletionItemKind::FIELD,
        K::Local => CompletionItemKind::VARIABLE,
        K::Script => CompletionItemKind::CLASS,
        K::Type => CompletionItemKind::STRUCT,
        K::Flag => CompletionItemKind::KEYWORD,
        K::State => CompletionItemKind::ENUM_MEMBER,
        K::Snippet => CompletionItemKind::SNIPPET,
    }
}

fn symbol_kind(kind: papyrus_ide::SymbolKind) -> lsp_types::SymbolKind {
    use papyrus_ide::SymbolKind as K;
    match kind {
        K::Script => lsp_types::SymbolKind::CLASS,
        K::Function => lsp_types::SymbolKind::FUNCTION,
        K::Event => lsp_types::SymbolKind::EVENT,
        K::Property => lsp_types::SymbolKind::PROPERTY,
        K::Variable => lsp_types::SymbolKind::FIELD,
        K::State => lsp_types::SymbolKind::NAMESPACE,
    }
}

fn document_symbol(lines: &Lines, s: papyrus_ide::DocumentSymbol) -> lsp_types::DocumentSymbol {
    let range = lines.range(s.full_range);
    let mut selection = lines.range(s.focus_range);
    // The selection range must be contained in the full range.
    if selection.start < range.start || selection.end > range.end {
        selection = range;
    }
    #[allow(deprecated)]
    lsp_types::DocumentSymbol {
        name: if s.name.is_empty() { "?".to_owned() } else { s.name },
        detail: s.detail,
        kind: symbol_kind(s.kind),
        tags: None,
        deprecated: None,
        range,
        selection_range: selection,
        children: Some(s.children.into_iter().map(|c| document_symbol(lines, c)).collect()),
    }
}
