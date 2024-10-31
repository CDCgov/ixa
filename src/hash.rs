use std::hash::{DefaultHasher, Hash, Hasher};

pub struct Hasher128 {
    buf: Vec<u8>,
}

impl Hasher128 {
    fn new() -> Self {
        Hasher128 { buf: Vec::new() }
    }

    fn finish_128(&self) -> u128 {
        println!("Len {}", self.buf.len());
        println!("{:?}", self.buf);
        if self.buf.len() <= 16 {
            let mut tmp: [u8; 16] = [0; 16];
            tmp[..self.buf.len()].copy_from_slice(&self.buf[..]);
            return u128::from_le_bytes(tmp);
        }

        let mut hasher = DefaultHasher::new();
        hasher.write_u32(0x5c5_c5c5c);
        self.buf.hash(&mut hasher);
        let h1 = hasher.finish();
        let mut hasher = DefaultHasher::new();
        hasher.write_u32(0x3636_3636);
        self.buf.hash(&mut hasher);
        let h2 = hasher.finish();
        let tmp: u128 = h1.into();
        tmp << 64 | u128::from(h2)
    }
}

impl Hasher for Hasher128 {
    fn write(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn finish(&self) -> u64 {
        panic!("Unimplemented")
    }
}

#[allow(clippy::module_name_repetitions)]
pub fn hash_ref<T: Hash>(val: &T) -> u128 {
    let mut hasher = Hasher128::new();
    val.hash(&mut hasher);
    hasher.finish_128()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hasher128_new() {
        let hasher = Hasher128::new();
        assert!(hasher.buf.is_empty());
    }

    #[test]
    fn test_hasher128_write() {
        let mut hasher = Hasher128::new();
        hasher.write(b"hello");
        assert_eq!(hasher.buf, b"hello");
    }

    #[test]
    fn test_hasher128_finish2_short() {
        let mut hasher = Hasher128::new();
        hasher.write(b"short");
        let result = hasher.finish_128();
        assert_eq!(
            result,
            u128::from_le_bytes([115, 104, 111, 114, 116, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        );
    }

    #[test]
    fn test_hasher128_finish2_long() {
        let mut hasher = Hasher128::new();
        hasher.write(b"this is a longer string that exceeds 16 bytes");
        let result = hasher.finish_128();
        assert!(result > 0);
    }

    #[test]
    fn test_hash_ref_same_values() {
        let value = "test value";
        let value2 = "test value";
        assert_eq!(hash_ref(&value), hash_ref(&value2));
    }

    #[test]
    fn test_hash_ref_different_values() {
        let value1 = 42;
        let value2 = 43;
        let hash1 = hash_ref(&value1);
        let hash2 = hash_ref(&value2);
        assert_ne!(hash1, hash2);
    }
}
