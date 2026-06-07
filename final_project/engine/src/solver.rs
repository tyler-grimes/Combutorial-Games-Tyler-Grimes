use crate::board::{Position, WIDTH, HEIGHT};

pub(crate) const TT_SIZE: usize = 4_194_301; // prime < 2^22
const OFFSET: i8 = 64; // shift scores so 0 can mean "vacant"

pub(crate) struct TranspositionTable {
    entries: Vec<(u64, i8)>,
}

impl TranspositionTable {
    pub fn new() -> Self {
        TranspositionTable { entries: vec![(0, 0); TT_SIZE] }
    }

    fn index(key: u64) -> usize {
        (key % TT_SIZE as u64) as usize
    }

    pub fn put(&mut self, key: u64, value: i8) {
        self.entries[Self::index(key)] = (key, value + OFFSET);
    }

    pub fn get(&self, key: u64) -> Option<i8> {
        let (k, v) = self.entries[Self::index(key)];
        if v != 0 && k == key {
            Some(v - OFFSET)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.entries.iter_mut().for_each(|e| *e = (0, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tt_put_get_roundtrip() {
        let mut tt = TranspositionTable::new();
        tt.put(12345, -3);
        assert_eq!(tt.get(12345), Some(-3));
        assert_eq!(tt.get(99999), None);
    }

    #[test]
    fn tt_collision_overwrites() {
        let mut tt = TranspositionTable::new();
        let k1 = 7u64;
        let k2 = 7u64 + TT_SIZE as u64; // same slot
        tt.put(k1, 5);
        tt.put(k2, -5);
        assert_eq!(tt.get(k2), Some(-5));
        assert_eq!(tt.get(k1), None); // overwritten
    }
}
