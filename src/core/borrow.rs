use crate::span::Span;

/// Track borrow state with location information for precise diagnostics.
#[derive(Debug, Clone)]
pub(crate) enum BorrowState {
    Unborrowed,
    BorrowedImm { span: Span },
    BorrowedMut { span: Span },
}

impl BorrowState {
    #[allow(dead_code)]
    pub(crate) fn is_borrowed(&self) -> bool {
        !matches!(self, BorrowState::Unborrowed)
    }
}
