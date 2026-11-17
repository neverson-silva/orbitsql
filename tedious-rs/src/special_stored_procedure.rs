#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum SpecialStoredProcedure {
    SpCursor = 1,
    SpCursorOpen = 2,
    SpCursorPrepare = 3,
    SpCursorExecute = 4,
    SpCursorPrepExec = 5,
    SpCursorUnprepare = 6,
    SpCursorFetch = 7,
    SpCursorOption = 8,
    SpCursorClose = 9,
    SpExecuteSql = 10,
    SpPrepare = 11,
    SpExecute = 12,
    SpPrepExec = 13,
    SpPrepExecRpc = 14,
    SpUnprepare = 15,
}

impl SpecialStoredProcedure {
    pub fn id(self) -> u16 {
        self as u16
    }
}
