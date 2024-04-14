use std::time::{SystemTime, UNIX_EPOCH};


pub fn get_ms_timestamp() -> u64 {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).expect("Clock may have gone backwards");
    duration.as_millis() as u64
}