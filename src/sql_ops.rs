use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::fs;
use rusqlite::Connection;

const SQL_FILE_LOCATION: &str = "./sqheavy/db.sqlite";

pub enum SQLInitError {
    ReadInitFileFailed(String),
    DBConnectionFailed,
    ExecutionFailed(rusqlite::Error),
}
impl std::fmt::Display for SQLInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SQLInitError::ReadInitFileFailed(path_given) => write!(f, "Failed to read init.sql file. {} exists?", path_given),
            SQLInitError::DBConnectionFailed => write!(f, "Failed to connect to the database."),
            SQLInitError::ExecutionFailed(e) => write!(f, "SQL execution failed: {}", e),
        }
    }
}
pub fn init_sqlite_db(where_is_init_dot_sql: &str) -> Result<(), SQLInitError> {
    // read init.sql
    let init_sql_content = match fs::read_to_string(where_is_init_dot_sql) {
        Ok(content) => content,
        Err(_) => return Err(SQLInitError::ReadInitFileFailed(where_is_init_dot_sql.to_string())),
    };

    // connect to db
    let conn = match Connection::open(&SQL_FILE_LOCATION) {
        Ok(c) => c,
        Err(_) => return Err(SQLInitError::DBConnectionFailed),
    };

    // do init.sql
    match conn.execute_batch(&init_sql_content) {
        Ok(_) => Ok(()),
        Err(e) => Err(SQLInitError::ExecutionFailed(e)),
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


pub enum SQLInsertError {
    InvalidTable(String),
    EmptyFuzzyResult,
    DBConnectionFailed,
    ExecutionFailed(rusqlite::Error),
}
impl std::fmt::Display for SQLInsertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SQLInsertError::InvalidTable(wrong_table_name) => write!(f, "Invalid table name provided: {}", wrong_table_name),
            SQLInsertError::EmptyFuzzyResult => write!(f, "Fuzzy result cannot be empty."),
            SQLInsertError::DBConnectionFailed => write!(f, "Failed to connect to the database."),
            SQLInsertError::ExecutionFailed(e) => write!(f, "SQL execution failed: {}", e),
        }
    }
}

// table: vehicle_fuzzy_results / pedestrian_fuzzy_results
const TABLE_LIST: [&str; 2] = ["vehicle_fuzzy_results", "pedestrian_fuzzy_results"];
// fuzzy_result: recommended but not restricted, LOW / MEDIUM / HIGH
pub fn insert(table: &str, fuzzy_result :&str) -> Result<(), SQLInsertError> {
    // table MUST whitelist, fuzzy_result not empty
    if !TABLE_LIST.contains(&table) {
        return Err(SQLInsertError::InvalidTable(table.to_string()));
    }
    if fuzzy_result.is_empty() {
        return Err(SQLInsertError::EmptyFuzzyResult);
    }
    // connect to db
    let conn = match init_connection(SQL_FILE_LOCATION) {
        Some(c) => c,
        None => return Err(SQLInsertError::DBConnectionFailed),
    };
    // INSERT INTO table <fuzzy_result> VALUES <fuzzy_result>;
    let sql: String = format!("INSERT INTO {} (fuzzy_result, created_at) VALUES (?1, ?2)", table);
    let time_now: i64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    match conn.execute(&sql, rusqlite::params![fuzzy_result, time_now]) {
        Ok(_) => Ok(()),
        Err(e) => Err(SQLInsertError::ExecutionFailed(e)),
    }
}

