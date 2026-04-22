//! Base62 encoder for compact tag IDs.
//!
//! Alphabet: 0-9 a-z A-Z — стабильно сортируется, читаемо.
//! n=1 → 62 тега, n=2 → 3844, n=3 → 238328.

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const BASE: u64 = 62;

/// Encode non-negative integer to base62 string.
pub fn encode(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut buf = Vec::with_capacity(4);
    while n > 0 {
        buf.push(ALPHABET[(n % BASE) as usize]);
        n /= BASE;
    }
    buf.reverse();
    String::from_utf8(buf).expect("base62 chars are valid utf8")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_encode() {
        assert_eq!(encode(0), "0");
        assert_eq!(encode(61), "Z");
        assert_eq!(encode(62), "10");
        assert_eq!(encode(3843), "ZZ");
    }
}
