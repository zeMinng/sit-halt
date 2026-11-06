use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::engine::Engine;

/// 全局应用状态：数据库连接池 + 引擎句柄，注入到所有 command。
pub struct AppState {
    pub db: SqlitePool,
    pub db_path: PathBuf,
    pub engine: Arc<Engine>,
}
