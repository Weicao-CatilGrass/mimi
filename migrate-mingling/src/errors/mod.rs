use serde::Serialize;
use mingling::Groupped;

// ── 通用错误类型 ──────────────────────────────────────

/// 文件不存在或无法读取
#[derive(Groupped, Debug, Clone, Serialize)]
pub struct ErrorFileRead {
    pub path: String,
    pub detail: String,
}

/// 无法定位源文件（没传路径且没找到 mimi.toml）
#[derive(Groupped, Debug, Clone, Serialize)]
pub struct ErrorSourceResolve {
    pub detail: String,
}

/// 词法/语法分析失败
#[derive(Groupped, Debug, Clone, Serialize)]
pub struct ErrorParse {
    pub path: String,
    pub detail: String,
}
