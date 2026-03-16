use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::fs;
use rusqlite::Connection;

// sqlite but sqheavy, need a tutorial of understand what Tails doing
// Like we are hanging out and making perler beads, we talked about topics from the sky to the underground
// Once upon a time in a discord vc we did it, and they asked us for a tutorial of "Understanding Tails 101" since they know exactly every word means but zero clue about the whole sentence
// But I chose to insist it and keep it as a remarkable and unique feature of Tails' workspace
// You can find our essays in literally every project and git shows who the author is
// In some way this is good, cuz when your tutor ask did we use AI to generate the code, I could show them the commit history and essays here
// and say, hey, AI would not do such a thing
// AIs are kind of stupid, they only know the possibility, but they don't know how to have some fun in comments
// If they asked about it, I would explain in this way, yolo, have fun
// Not that YOLO, but sure, we are using YOLO for object detection, huh
// I guess if they are not familiar with English and just take a glance they would consider this essay a description of the file or something, but when they take a closer look it would be rickroll
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

pub enum SQLSelectError {
    InvalidTable(String),
    DBConnectionFailed,
    ExecutionFailed(rusqlite::Error),
}
// get average object count
// how to do it: time - 24 hours, get the closest record with 1 before and 1 after, avg 3 as the result of the day
// repeat this for 7 times and get 7 days' avg, then avg them to get a final result
// the final result will be sent to fuzzy_inference
pub fn get_avg_obj_count(table: &str) -> Result<i32, SQLSelectError> {
    // table MUST whitelist
    if !TABLE_LIST.contains(&table) {
        return Err(SQLSelectError::InvalidTable(table.to_string()));
    }
    let conn = match init_connection(SQL_FILE_LOCATION) {
        Some(c) => c,
        None => return Err(SQLSelectError::DBConnectionFailed),
    };

    let now: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut week_sum: f64 = 0.0;
    let mut day_count: i32 = 0;

    // Repeat for 7 days: target is now - day * 24h
    for day in 1..=7 {
        let target_timestamp = now - (day as i64) * 24 * 60 * 60;

        // 1. Find the single record whose created_at is closest to target_timestamp
        let closest_sql = format!(
            "SELECT rowid, object_count, created_at
             FROM {}
             ORDER BY ABS(created_at - ?1) ASC
             LIMIT 1",
            table
        );

        let closest: Result<(i64, i32, i64), rusqlite::Error> = conn.query_row(
            &closest_sql,
            rusqlite::params![target_timestamp],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );

        let (center_rowid, center_count, _center_created_at) = match closest {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => continue, // no data for this day anchor
            Err(e) => return Err(SQLSelectError::ExecutionFailed(e)),
        };

        // 2. Find 1 prev and 1 after record
        let prev_sql = format!(
            "SELECT object_count FROM {} WHERE rowid < ?1 ORDER BY rowid DESC LIMIT 1",
            table
        );
        let next_sql = format!(
            "SELECT object_count FROM {} WHERE rowid > ?1 ORDER BY rowid ASC LIMIT 1",
            table
        );

        // IDE is wrong, we used duplicated function because we need to ask for query twice and handle errors separately
        let prev_count: Option<i32> = match conn.query_row(
            &prev_sql,
            rusqlite::params![center_rowid],
            |row| row.get(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(SQLSelectError::ExecutionFailed(e)),
        };

        let next_count: Option<i32> = match conn.query_row(
            &next_sql,
            rusqlite::params![center_rowid],
            |row| row.get(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(SQLSelectError::ExecutionFailed(e)),
        };

        // 3. cal day avg
        // convert obj count to f64 for avg, it would be converted back to i32 at the end
        let mut sum = center_count as i64;
        let mut n :i32 = 1;
        // If no prev or next, we would not send 0 or null into avg process
        if let Some(v) = prev_count {
            sum += v as i64;
            n += 1;
        }
        if let Some(v) = next_count {
            sum += v as i64;
            n += 1;
        }

        let day_avg = sum as f64 / n as f64;
        if day_avg == 0.0 {
            continue; // skip zero results, do not count the day
        }

        week_sum += day_avg;
        day_count += 1;
    }

    if day_count == 0 {
        return Ok(0);
    }

    let week_avg = week_sum / day_count as f64;
    Ok(week_avg.round() as i32)
    // Different from c++, it would round to the nearest int, but not flooring
    // That was unexpected, we are feeling better with rust
}