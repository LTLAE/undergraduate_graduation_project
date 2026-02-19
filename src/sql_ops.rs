use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::fs;
use rusqlite::Connection;

const SQL_FILE_LOCATION: &str = "./sqheavy/db.sqlite";

pub fn init_sqlite_db(where_is_init_dot_sql: &str) -> bool {
    // read init.sql
    let init_sql_content = match fs::read_to_string(where_is_init_dot_sql) {
        Ok(content) => content,
        Err(_) => return false,
    };

    // 创建数据库连接（如果文件不存在会自动创建）
    let conn = match Connection::open(&SQL_FILE_LOCATION) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // 执行 SQL 初始化语句
    match conn.execute_batch(&init_sql_content) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn init_connection(where_is_db_file: &str) -> Option<Connection> {
    match Connection::open(where_is_db_file) {
        Ok(conn) => Some(conn),
        Err(_) => None,
    }
}

fn disconnect(connection: Connection) -> bool {
    drop(connection);
    true
}

// table: vehicle_fuzzy_results / pedestrian_fuzzy_results
const TABLE_LIST: [&str; 2] = ["vehicle_fuzzy_results", "pedestrian_fuzzy_results"];
// fuzzy_result: recommended but not restricted, LOW / MEDIUM / HIGH
pub fn insert(table: &str, fuzzy_result :&str) -> bool {
    // table and fuzzy_result must not be null
    if !TABLE_LIST.contains(&table) {
        false;
    }
    if fuzzy_result.is_empty() {
        false;
    }
    // connect to db
    let conn = init_connection(SQL_FILE_LOCATION);
    if conn.is_none() {
        false;
    }







    true
}