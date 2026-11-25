//! The external-library scanner: what the blueprint references but does
//! not carry.
//!
//! The blueprint names each file's owning library, and the files
//! themselves name every library their clauses reach (`use <lib>.<unit>`,
//! `library <lib>;`). The difference -- references whose library is
//! neither carried nor predefined -- is the set a file-oriented tool
//! must find elsewhere: a precompiled vendor library (unisim and
//! friends) a design reaches into but the dependency graph never
//! provided.
//!
//! The scan is grep-grade by design and its verdicts gate wiring and
//! hints only: it never rejects a file, reorders rows, or validates a
//! provision. Comment-stripped lines split on whitespace; string
//! literals containing clause-shaped text are not modeled.

use crate::blueprint::Entry;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The libraries every HDL tool defines on its own; never external.
pub const PREDEFINED_LIBS: [&str; 3] = ["ieee", "std", "work"];

/// The sorted, deduped, lowercase set of libraries the entries' files
/// reference through `use`/`library` clauses but neither the entries'
/// own `library` fields nor [`PREDEFINED_LIBS`] provide. Unreadable
/// files contribute nothing.
pub fn external_libraries(entries: &[Entry]) -> Vec<String> {
    let mut carried: HashSet<String> = entries
        .iter()
        .map(|e| e.library.to_lowercase())
        .chain(PREDEFINED_LIBS.iter().map(|s| s.to_string()))
        .collect();
    let mut referenced: HashSet<String> = HashSet::new();
    for entry in entries {
        let data = match fs::read(&entry.path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for line in String::from_utf8_lossy(&data).lines() {
            let code = match line.find("--") {
                Some(i) => &line[..i],
                None => line,
            };
            let tokens: Vec<&str> = code.split_whitespace().collect();
            for (idx, token) in tokens.iter().enumerate() {
                // A clause's names run comma-separated to the terminator:
                // `library ieee, unisim;`, `use a.pkg, b.pkg;`. Each
                // operand yields its library head; the ';' token ends the
                // clause.
                let head = if token.eq_ignore_ascii_case("use") {
                    Some('.')
                } else if token.eq_ignore_ascii_case("library") {
                    Some(';')
                } else {
                    None
                };
                if let Some(sep) = head {
                    for operand in &tokens[idx + 1..] {
                        let name = if sep == '.' {
                            operand.split('.').next().unwrap_or("")
                        } else {
                            operand.trim_end_matches([';', ',']).trim()
                        };
                        if !name.is_empty() {
                            referenced.insert(name.to_lowercase());
                        }
                        if operand.contains(';') {
                            break;
                        }
                    }
                }
            }
        }
    }
    let mut external: Vec<String> = referenced
        .into_iter()
        .filter(|name| !carried.contains(name))
        .collect();
    external.sort();
    external
}

/// Writes `content` to a fresh fixture file under the test-name-keyed
/// scratch directory (the parked pid-keyed collision: key by test name).
#[cfg(test)]
fn fixture(name: &str, file: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(concat!("orbit_vendor_", env!("CARGO_PKG_NAME"))).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(file);
    fs::write(&path, content).unwrap();
    path
}

#[cfg(test)]
fn entry(library: &str, path: &Path) -> Entry {
    Entry {
        line: 1,
        fileset: "VHDL".to_string(),
        library: library.to_string(),
        path: path.to_path_buf(),
        std: None,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn predefined_set_is_the_three_libraries() {
        assert_eq!(PREDEFINED_LIBS, ["ieee", "std", "work"]);
    }

    #[test]
    fn mixed_case_unisim_references_surface_external() {
        let path = fixture(
            "external_case",
            "wrapper.vhd",
            "library IEEE;\nuse IEEE.std_logic_1164.all;\n\nlibrary UNISIM;\nuse UNISIM.VCOMPONENTS.all;\n\nentity wrapper is\nend;\n",
        );
        let entries = [entry("techmap", &path)];
        assert_eq!(external_libraries(&entries), ["unisim"]);
    }

    #[test]
    fn commented_references_do_not_count() {
        let path = fixture(
            "comment_only",
            "unit.vhd",
            "-- use unisim.something;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), Vec::<String>::new());
    }

    #[test]
    fn multi_name_clauses_split_on_commas() {
        let path = fixture(
            "multi_name",
            "unit.vhd",
            "library ieee, UNISIM;\nuse ieee.std_logic_1164.all, UNISIM.VCOMPONENTS.all;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), ["unisim"]);
    }

    #[test]
    fn carried_libraries_are_never_external() {
        let path = fixture(
            "local_ref",
            "unit.vhd",
            "use gaisler.arith.all;\nentity unit is\nend;\n",
        );
        let entries = [entry("gaisler", &path)];
        assert_eq!(external_libraries(&entries), Vec::<String>::new());
    }
}
