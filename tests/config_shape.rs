//! The shipped target fragment's shape, pinned textually: every
//! [[target]] entry carries the custom recursive ALLHDL fileset (the
//! owner's 2026-11-08 ruling) -- the FULL HDL file set of every project
//! in the dependency graph riding every blueprint regardless of orbit's
//! reference closure (probed live: a dependency project's unreferenced
//! source rides the blueprint under ALLHDL where the bare planning drops
//! it -- the default-bound entities, invisible to the reference walk).
//! The probed semantics the drives answer: a custom fileset REPLACES the
//! built-in VHDL/VLOG/SYSV collection in the blueprint, so every row
//! rides as ALLHDL and the drives route it by its file extension.

use std::fs;

const CONFIG: &str = "config.toml";

/// The target entries of the fragment: each `[[target]]` block's text,
/// in file order.
fn target_blocks() -> Vec<String> {
    let text = fs::read_to_string(CONFIG).unwrap();
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("[[target]]") {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(String::new());
        } else if current.is_some() {
            current.as_mut().unwrap().push_str(line);
            current.as_mut().unwrap().push('\n');
        }
    }
    if let Some(block) = current.take() {
        blocks.push(block);
    }
    blocks
}

#[test]
fn every_target_entry_carries_the_allhdl_fileset() {
    // The 14 shipped targets across the 8 carried tools; each one's
    // entry carries the custom recursive ALLHDL fileset, so the FULL HDL
    // file set rides every blueprint regardless of the reference
    // closure.
    let blocks = target_blocks();
    assert_eq!(blocks.len(), 14, "the fragment ships 14 target entries");
    for block in &blocks {
        let name = block
            .lines()
            .find(|line| line.starts_with("name = "))
            .unwrap_or("(no name)");
        assert!(
            block.contains("fileset.ALLHDL = { patterns = [\"**/*.vhd\", \"**/*.vhdl\", \"**/*.v\", \"**/*.sv\"], recursive = true }"),
            "the {name} target should carry the recursive ALLHDL fileset verbatim:\n{block}"
        );
    }
}

#[test]
fn the_fileset_line_rides_inside_each_target_entry() {
    // The fileset key lives INSIDE the [[target]] entry (immediately
    // after its `command`, before the next entry) -- the
    // creating_targets.md face: a fileset outside an entry is orbit's
    // global namespace, not the target's.
    let text = fs::read_to_string(CONFIG).unwrap();
    let mut entries = 0;
    for line in text.lines() {
        if line.starts_with("[[target]]") {
            entries += 1;
        }
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut adjacencies = 0;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("command = ")
            && lines
                .get(index + 1)
                .map(|next| next.starts_with("fileset.ALLHDL"))
                .unwrap_or(false)
        {
            adjacencies += 1;
        }
    }
    assert_eq!(
        adjacencies, entries,
        "every target entry's command should be immediately followed by its ALLHDL \
         fileset line"
    );
}

#[test]
fn the_header_documents_the_why_and_the_probed_semantics() {
    // The fragment's header carries the ruling's story: the closure
    // bypass, the probed replace-semantics, the extension routing.
    let text = fs::read_to_string(CONFIG).unwrap();
    let header = text
        .lines()
        .take_while(|line| line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        header.contains("fileset.ALLHDL"),
        "the header should name the fileset:\n{header}"
    );
    assert!(
        header.contains("bypasses the closure"),
        "the header should state the closure bypass:\n{header}"
    );
    assert!(
        header.contains("REPLACES the built-in"),
        "the header should state the probed replace-semantics:\n{header}"
    );
    assert!(
        header.contains("route ALLHDL rows by their"),
        "the header should state the extension routing:\n{header}"
    );
}
