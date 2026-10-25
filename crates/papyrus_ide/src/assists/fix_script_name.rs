//! Quick fix of `script-name-mismatch`: renames the script after its file.
//!
//! In `MyQuest.psc`:
//!
//! ```papyrus
//! ScriptName MyQuets extends Quest    ; → ScriptName MyQuest extends Quest
//! ```

use super::{Assist, AssistCtx, AssistKind, TextEdit};

pub(crate) fn fix_script_name(ctx: &AssistCtx<'_>, acc: &mut Vec<Assist>) {
    let index = ctx.sema.index;
    let (Some(name), Some(range)) = (index.name, index.name_range) else { return };
    if ctx.range.intersect(header_line(ctx, range)).is_none() {
        return;
    }
    let path = &ctx.sema.ws.file(ctx.sema.file).path;
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else { return };
    if stem.eq_ignore_ascii_case(name.as_str()) {
        return;
    }
    acc.push(Assist {
        id: "fix_script_name",
        label: format!("Rename the script to `{stem}`, like its file"),
        kind: AssistKind::QuickFix,
        target: range,
        edits: vec![TextEdit { range, new_text: stem }],
        fixes: Some("script-name-mismatch"),
    });
}

/// The `ScriptName` line, where the cursor may be.
fn header_line(ctx: &AssistCtx<'_>, name: papyrus_parser::TextRange) -> papyrus_parser::TextRange {
    super::line_range(ctx.sema.text, name)
}

#[cfg(test)]
mod tests {
    use super::fix_script_name;
    use crate::assists::test_utils::{check_assist_in, check_assist_not_applicable};

    #[test]
    fn renames_after_the_file() {
        check_assist_in(
            fix_script_name,
            "/ws/MyQuest.psc",
            "ScriptName My$0Quets extends Quest\n",
            "ScriptName MyQuest extends Quest\n",
        );
    }

    #[test]
    fn not_when_matching() {
        // The test file is `T.psc`.
        check_assist_not_applicable(fix_script_name, "ScriptName $0t\n");
    }
}
