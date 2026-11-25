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

/// Blanks string-literal contents and comments to spaces over the whole
/// text, preserving newlines: string literals may hold both `--` and
/// clause-shaped text, and a `--` inside a literal is not a comment. The
/// result tokenizes like the code, minus everything the scan must not
/// see.
fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    let _ = chars.next();
                    out.push_str("  ");
                } else {
                    in_string = false;
                    out.push(' ');
                }
            } else {
                out.push(if c == '\n' { '\n' } else { ' ' });
            }
        } else if c == '"' {
            in_string = true;
            out.push(' ');
        } else if c == '-' && chars.peek() == Some(&'-') {
            let _ = chars.next();
            out.push_str("  ");
            // a comment ends at the newline -- consuming further would
            // blank the rest of the file, clauses included
            while let Some(&rest) = chars.peek() {
                if rest == '\n' {
                    break;
                }
                out.push(' ');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The sorted, deduped, lowercase set of libraries the entries' files
/// reference through `use`/`library` clauses but neither the entries'
/// own `library` fields nor [`PREDEFINED_LIBS`] provide. Unreadable
/// files contribute nothing.
pub fn external_libraries(entries: &[Entry]) -> Vec<String> {
    let carried: HashSet<String> = entries
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
        let text = sanitize(&String::from_utf8_lossy(&data));
        let tokens: Vec<&str> = text.split_whitespace().collect();
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
                    // Configuration binding indications name entities
                    // and configurations, not libraries: `use entity
                    // work.comp(rtl);` reaches `work` (predefined), and
                    // `use open;` names nothing. The keywords ride
                    // their own tokens, so skipping them keeps the
                    // following selected name's head as the library.
                    let bare = operand.trim_end_matches([';', ',']);
                    let keyword = bare.eq_ignore_ascii_case("entity")
                        || bare.eq_ignore_ascii_case("configuration")
                        || bare.eq_ignore_ascii_case("open");
                    if !keyword {
                        let name = if sep == '.' {
                            operand.split('.').next().unwrap_or("")
                        } else {
                            operand.trim_end_matches([';', ',']).trim()
                        };
                        // a name must carry at least one alphanumeric:
                        // quoted-operator clauses (`use grlib.stdlib."+";`)
                        // blank to whitespace in the sanitizer and leave a
                        // bare terminator token that is never a library
                        if !name.is_empty()
                            && name.chars().any(|c| c.is_ascii_alphanumeric())
                        {
                            referenced.insert(name.to_lowercase());
                        }
                    }
                    if operand.contains(';') || bare.eq_ignore_ascii_case("open") {
                        break;
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
    let dir = std::env::temp_dir()
        .join(concat!("orbit_vendor_", env!("CARGO_PKG_NAME")))
        .join(name);
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
    fn quoted_operator_clauses_yield_no_phantom_names() {
        // the GRLIB idiom `use grlib.stdlib."+";`: the quoted operator
        // blanks to whitespace in the sanitizer, splitting a bare `;`
        // terminator token after `use` -- the terminator is never a
        // library name
        let path = fixture(
            "quoted_operator",
            "unit.vhd",
            "library gaisler;\nuse gaisler.l2c_lite.all;\nuse grlib.stdlib.\"+\";\nentity unit is\nend;\n",
        );
        let entries = [entry("gaisler", &path), entry("grlib", path.with_file_name("other.vhd").as_path())];
        assert_eq!(external_libraries(&entries), Vec::<String>::new());
    }

    #[test]
    fn comments_blank_only_their_own_line() {
        let path = fixture(
            "header_comment",
            "unit.vhd",
            "-- a header comment\nlibrary IEEE;\nuse IEEE.std_logic_1164.all;\nuse unisim.vcomponents.all;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), ["unisim"]);
    }

    #[test]
    fn string_literal_references_do_not_count() {
        let path = fixture(
            "string_literal",
            "unit.vhd",
            "report \" use unisim.vcomponents.all; \" severity note;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), Vec::<String>::new());
    }

    #[test]
    fn comment_marker_inside_string_is_not_a_comment() {
        let path = fixture(
            "comment_in_string",
            "unit.vhd",
            "constant s : string := \"--\"; use unisim.vcomponents.all;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), ["unisim"]);
    }

    #[test]
    fn multi_line_library_clause_is_found() {
        let path = fixture(
            "multi_line",
            "unit.vhd",
            "library\n  UNISIM;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), ["unisim"]);
    }

    #[test]
    fn externals_are_sorted() {
        let path = fixture(
            "sorted",
            "unit.vhd",
            "use zeta.lib.all;\nlibrary alpha;\nentity unit is\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), ["alpha", "zeta"]);
    }

    #[test]
    fn configuration_binding_indications_are_not_libraries() {
        let path = fixture(
            "config_binding",
            "cfg.vhd",
            "configuration cfg of unit is\n  for rtl\n    use entity work.comp(rtl);\n  end for;\nend;\n",
        );
        let entries = [entry("work", &path)];
        assert_eq!(external_libraries(&entries), Vec::<String>::new());
        let open_path = fixture(
            "config_open",
            "unit.vhd",
            "use open;\nentity unit is\nend;\n",
        );
        let open_entries = [entry("work", &open_path)];
        assert_eq!(external_libraries(&open_entries), Vec::<String>::new());
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
