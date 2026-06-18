use thiserror::Error;

pub type MimiResult<T> = std::result::Result<T, CompileError>;

#[derive(Debug, Error)]
pub enum CompileError {
    // === Variable/function resolution ===
    #[error("undefined variable '{0}'")]
    UndefinedVar(String),
    #[error("undefined function '{0}' in codegen")]
    UndefinedFunc(String),

    // === Type errors ===
    #[error("[E0703] actor '{0}' type is not a struct")]
    ActorNotStruct(String),
    #[error("[E0707] cannot access field on type '{0}'")]
    FieldAccessType(String),
    #[error("[E0708] cannot dispatch method '{method}' on {obj_type}")]
    MethodDispatch { method: String, obj_type: String },
    #[error("field '{field}' not found on type '{obj_type}'")]
    FieldNotFound { field: String, obj_type: String },
    #[error("[E0712] {0}")]
    TypeMismatch(String),
    #[error("type '{0}' is not a struct")]
    NotStruct(String),

    // === Argument errors ===
    #[error("[E0711] {0}")]
    WrongArgCount(String),
    #[error("[E0720] turbofish for '{name}' expects {expected} type args, got {found}")]
    TurbofishArgCount { name: String, expected: usize, found: usize },

    // === Capabilities ===
    #[error("[E0718] capability '{0}' has already been consumed")]
    CapConsumed(String),
    #[error("linear capability '{0}' must be consumed (via drop) before end of scope")]
    CapNotConsumed(String),

    // === Platform ===
    #[error("[E0750] '{0}' requires libc (not available in no_std mode)")]
    RequiresLibc(String),

    // === Expression/operator errors ===
    #[error("unsupported binary operator {0:?}")]
    UnsupportedBinOp(String),
    #[error("unsupported expression in codegen: {0:?}")]
    UnsupportedExpr(String),
    #[error("cannot call {0}: expected a function or closure")]
    NotCallable(String),

    // === Contracts ===
    #[error("contract condition must be boolean, got {0:?}")]
    ContractCondition(String),

    // === Loop control ===
    #[error("break outside of loop")]
    BreakOutsideLoop,
    #[error("continue outside of loop")]
    ContinueOutsideLoop,

    // === Runtime errors ===
    #[error("assertion failed: {0}")]
    AssertionFailed(String),
    #[error("index out of bounds: index {index} is not valid for {kind} of length {len}")]
    OutOfBounds { index: i64, len: usize, kind: String },
    #[error("division by zero")]
    DivByZero,
    #[error("modulo by zero")]
    ModByZero,

    // === FFI ===
    #[error("FFI wrapper: {0}")]
    FfiWrapper(String),

    // === I/O ===
    #[error("I/O error: {0}")]
    Io(String),

    // === Generic catch-all ===
    #[error("{0}")]
    Generic(String),
}

impl From<String> for CompileError {
    fn from(msg: String) -> Self {
        CompileError::Generic(msg)
    }
}

impl From<&str> for CompileError {
    fn from(msg: &str) -> Self {
        CompileError::Generic(msg.to_string())
    }
}
