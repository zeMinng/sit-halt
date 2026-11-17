use serde::{Serialize, Serializer};

/// 统一的后端错误类型：所有 command 都返回 AppResult，错误信息可序列化给前端。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库操作失败：{0}")]
    Db(#[from] sqlx::Error),

    #[error("数据库迁移失败：{0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("当前状态不允许该操作：{0}")]
    State(String),

    #[error("参数不合法：{0}")]
    Invalid(String),

    #[error("文件操作失败：{0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
