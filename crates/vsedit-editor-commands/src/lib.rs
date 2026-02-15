//! Core editor editing commands.

/// Standard editing commands.
pub enum EditorCommand {
    Undo, Redo,
    Cut, Copy, Paste,
    SelectAll, Delete, Backspace,
    IndentLine, OutdentLine,
    InsertLineBefore, InsertLineAfter,
    MoveLinesUp, MoveLinesDown,
    CopyLinesUp, CopyLinesDown,
    DeleteLine, JoinLines,
    ToggleComment, ToggleBlockComment,
    TransformUppercase, TransformLowercase,
}

impl EditorCommand {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Cut => "editor.action.clipboardCutAction",
            Self::Copy => "editor.action.clipboardCopyAction",
            Self::Paste => "editor.action.clipboardPasteAction",
            Self::SelectAll => "editor.action.selectAll",
            Self::Delete => "deleteRight",
            Self::Backspace => "deleteLeft",
            Self::IndentLine => "editor.action.indentLines",
            Self::OutdentLine => "editor.action.outdentLines",
            Self::InsertLineBefore => "editor.action.insertLineBefore",
            Self::InsertLineAfter => "editor.action.insertLineAfter",
            Self::MoveLinesUp => "editor.action.moveLinesUpAction",
            Self::MoveLinesDown => "editor.action.moveLinesDownAction",
            Self::CopyLinesUp => "editor.action.copyLinesUpAction",
            Self::CopyLinesDown => "editor.action.copyLinesDownAction",
            Self::DeleteLine => "editor.action.deleteLines",
            Self::JoinLines => "editor.action.joinLines",
            Self::ToggleComment => "editor.action.commentLine",
            Self::ToggleBlockComment => "editor.action.blockComment",
            Self::TransformUppercase => "editor.action.transformToUppercase",
            Self::TransformLowercase => "editor.action.transformToLowercase",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_ids() {
        assert_eq!(EditorCommand::Undo.id(), "undo");
        assert_eq!(EditorCommand::Copy.id(), "editor.action.clipboardCopyAction");
        assert_eq!(EditorCommand::ToggleComment.id(), "editor.action.commentLine");
    }
}
