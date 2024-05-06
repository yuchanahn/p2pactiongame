pub fn plus(a: u64, b: i32) -> u64 {
    if b < 0 {
        a - b.abs() as u64
    } else {
        a + b as u64
    }
}

pub fn minus(a: u64, b: u64) -> u64 {
    if b > a {
        0
    } else {
        a - b
    }
}