use crate::board::Position;
use crate::solver::Solver;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

const MAGIC: u32 = 0x4334424B; // "C4BK"

pub struct Book {
    depth: u8,
    entries: Vec<(u64, i8)>, // sorted by canonical key
}

impl Book {
    pub fn depth(&self) -> u8 {
        self.depth
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut f = std::fs::File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "bad book file");
        if buf.len() < 13 || u32::from_le_bytes(buf[0..4].try_into().unwrap()) != MAGIC {
            return Err(bad());
        }
        let depth = buf[4];
        let count = u64::from_le_bytes(buf[5..13].try_into().unwrap()) as usize;
        if buf.len() != 13 + count * 9 {
            return Err(bad());
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let off = 13 + i * 9;
            let key = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
            entries.push((key, buf[off + 8] as i8));
        }
        Ok(Book { depth, entries })
    }

    pub fn lookup(&self, p: &Position) -> Option<i8> {
        let key = p.canonical_key();
        self.entries
            .binary_search_by_key(&key, |e| e.0)
            .ok()
            .map(|i| self.entries[i].1)
    }
}

pub fn generate(path: &Path, depth: u8) -> std::io::Result<()> {
    let mut solved: BTreeMap<u64, i8> = BTreeMap::new();
    let mut solver = Solver::new();

    fn walk(
        p: &Position,
        remaining: u8,
        solved: &mut BTreeMap<u64, i8>,
        solver: &mut Solver,
    ) {
        if remaining == 0 {
            return;
        }
        for col in 0..crate::board::WIDTH {
            if !p.can_play(col) || p.is_winning_move(col) {
                continue;
            }
            let mut child = *p;
            child.play(col);
            let key = child.canonical_key();
            if let std::collections::btree_map::Entry::Vacant(e) = solved.entry(key) {
                let score = solver.solve(&child) as i8;
                e.insert(score);
                if solved.len().is_multiple_of(1000) {
                    eprintln!("solved {} positions", solved.len());
                }
            }
            walk(&child, remaining - 1, solved, solver);
        }
    }
    walk(&Position::new(), depth, &mut solved, &mut solver);

    let mut out = Vec::with_capacity(13 + solved.len() * 9);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(depth);
    out.extend_from_slice(&(solved.len() as u64).to_le_bytes());
    for (k, v) in &solved {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(*v as u8);
    }
    std::fs::File::create(path)?.write_all(&out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{board::Position, solver::Solver};

    #[test]
    #[ignore] // slow: solves near-empty boards; run with: cargo test -p engine --release -- --ignored
    fn book_roundtrip_and_lookup() {
        let dir = std::env::temp_dir().join("c4book_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("d2.book");

        generate(&path, 2).unwrap();
        let book = Book::load(&path).unwrap();
        assert_eq!(book.depth(), 2);

        let mut s = Solver::new();
        for c1 in 0..7usize {
            let mut p = Position::new();
            p.play(c1);
            assert_eq!(book.lookup(&p), Some(s.solve(&p) as i8), "depth1 col {c1}");
            for c2 in 0..7usize {
                let mut q = p;
                if !q.can_play(c2) {
                    continue;
                }
                // If c2 is a winning move, book skips it (game would be over)
                if p.is_winning_move(c2) {
                    continue;
                }
                q.play(c2);
                assert_eq!(book.lookup(&q), Some(s.solve(&q) as i8));
            }
        }
        let p3 = Position::from_moves("444").unwrap();
        assert_eq!(book.lookup(&p3), None);
    }

    #[test]
    #[ignore] // slow: solves near-empty boards; run with: cargo test -p engine --release -- --ignored
    fn lookup_is_mirror_invariant() {
        let dir = std::env::temp_dir().join("c4book_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("d1.book");
        generate(&path, 1).unwrap();
        let book = Book::load(&path).unwrap();
        let a = Position::from_moves("1").unwrap();
        let b = Position::from_moves("7").unwrap();
        assert_eq!(book.lookup(&a), book.lookup(&b));
        assert!(book.lookup(&a).is_some());
    }
}
