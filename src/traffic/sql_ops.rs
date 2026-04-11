use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::fs;
use rusqlite::Connection;
use crate::config::SQL_FILE_LOCATION;

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

#[derive(Debug, Clone)]
pub enum SQLError {
    InvalidTable(String),
    EmptyFuzzyResult,
    ReadInitFileFailed(String),
    DBConnectionFailed,
    ExecutionFailed(String),  // Changed from rusqlite::Error to String for Clone support
}

impl std::fmt::Display for SQLError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SQLError::InvalidTable(table_name) => write!(f, "Invalid table name: {}", table_name),
            SQLError::EmptyFuzzyResult => write!(f, "Fuzzy result cannot be empty."),
            SQLError::ReadInitFileFailed(path_given) => write!(f, "Failed to read init.sql file. {} exists?", path_given),
            SQLError::DBConnectionFailed => write!(f, "Failed to connect to the database."),
            SQLError::ExecutionFailed(e) => write!(f, "SQL execution failed: {}", e),
        }
    }
}
pub(crate) fn init_sqlite_db(where_is_init_dot_sql: &str) -> Result<(), SQLError> {
    // read init.sql
    let init_sql_content = match fs::read_to_string(where_is_init_dot_sql) {
        Ok(content) => content,
        Err(_) => return Err(SQLError::ReadInitFileFailed(where_is_init_dot_sql.to_string())),
    };

    // connect to db
    let conn = match Connection::open(&SQL_FILE_LOCATION) {
        Ok(c) => c,
        Err(_) => return Err(SQLError::DBConnectionFailed),
    };

    // do init.sql
    match conn.execute_batch(&init_sql_content) {
        Ok(_) => Ok(()),
        Err(e) => Err(SQLError::ExecutionFailed(e.to_string())),
    }
}

fn init_connection(where_is_db_file: &str) -> Result<Connection, SQLError> {
    Connection::open(where_is_db_file).map_err(|_| SQLError::DBConnectionFailed)
}

const TABLE_LIST: [&str; 2] = ["vehicle_fuzzy_results", "pedestrian_fuzzy_results"];
const VEHICLE_TABLE: &str = "vehicle_fuzzy_results";
const PEDESTRIAN_TABLE: &str = "pedestrian_fuzzy_results";

fn insert_count(table: &str, obj_count: i32) -> Result<(), SQLError> {
    println!("Try inserting into table: {}, obj_count: {}", table, obj_count);
    // table MUST whitelist, fuzzy_result not empty
    if !TABLE_LIST.contains(&table) {
        return Err(SQLError::InvalidTable(table.to_string()));
    }
    if obj_count < 0 {
        return Err(SQLError::EmptyFuzzyResult);
    }
    // connect to db
    let conn = init_connection(SQL_FILE_LOCATION)?;
    // INSERT INTO table <fuzzy_result> VALUES <fuzzy_result>;
    let sql: String = format!("INSERT INTO {} (obj_count, timestamp) VALUES (?1, ?2)", table);
    let time_now: i64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    conn.execute(&sql, rusqlite::params![obj_count, time_now])
        .map(|_| ())
        .map_err(|e| SQLError::ExecutionFailed(e.to_string()))
}

pub(crate) fn insert_pedestrian_count(obj_count: i32) -> Result<(), SQLError> {
    insert_count(PEDESTRIAN_TABLE, obj_count)
}

pub(crate) fn insert_vehicle_count(obj_count: i32) -> Result<(), SQLError> {
    insert_count(VEHICLE_TABLE, obj_count)
}

// get average object count
// how to do it: time - 24 hours, get the closest record with 1 before and 1 after, avg 3 as the result of the day
// repeat this for 7 times and get 7 days' avg, then avg them to get a final result
// the final result will be sent to fuzzy_inference
fn get_avg_obj_count(table: &str) -> Result<i32, SQLError> {
    println!("Try getting average object count from table: {}", table);
    // table MUST whitelist
    if !TABLE_LIST.contains(&table) {
        return Err(SQLError::InvalidTable(table.to_string()));
    }

    let conn = init_connection(SQL_FILE_LOCATION)?;

    // cleanup and propagate error
    cleanup(&conn)?;

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
            "SELECT rowid, obj_count, timestamp
             FROM {}
             ORDER BY ABS(timestamp - ?1) ASC
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
            Err(e) => return Err(SQLError::ExecutionFailed(e.to_string())),
        };

        // 2. Find 1 prev and 1 after record
        let prev_sql = format!(
            "SELECT obj_count FROM {} WHERE rowid < ?1 ORDER BY rowid DESC LIMIT 1",
            table
        );
        let next_sql = format!(
            "SELECT obj_count FROM {} WHERE rowid > ?1 ORDER BY rowid ASC LIMIT 1",
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
            Err(e) => return Err(SQLError::ExecutionFailed(e.to_string())),
        };

        let next_count: Option<i32> = match conn.query_row(
            &next_sql,
            rusqlite::params![center_rowid],
            |row| row.get(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(SQLError::ExecutionFailed(e.to_string())),
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

pub(crate) fn get_avg_pedestrian_count() -> Result<i32, SQLError> {
    get_avg_obj_count(PEDESTRIAN_TABLE)
}

pub(crate) fn get_avg_vehicle_count() -> Result<i32, SQLError> {
    get_avg_obj_count(VEHICLE_TABLE)
}

// Clear old records, keep only the latest 7 days' data, run once an hour
static LAST_CLEANUP: Lazy<Mutex<std::time::SystemTime>> = Lazy::new(|| Mutex::new(std::time::SystemTime::now()));
fn cleanup(conn: &Connection) -> Result<(), SQLError> {
    println!("Try cleaning up old records");
    let now = std::time::SystemTime::now();
    let mut last_cleanup = LAST_CLEANUP.lock().unwrap();
    if now.duration_since(*last_cleanup).unwrap_or_default().as_secs() < 3600 {
        return Ok(()); // not time to clean up yet
    }

    let seven_days_ago = now - std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let seven_days_ago_timestamp = seven_days_ago.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    for table in &TABLE_LIST {
        let sql = format!("DELETE FROM {} WHERE timestamp < ?1", table);
        match conn.execute(&sql, rusqlite::params![seven_days_ago_timestamp]) {
            Ok(_) => (),
            Err(e) => return Err(SQLError::ExecutionFailed(e.to_string())),
        }
    }

    *last_cleanup = now;
    Ok(())
}
