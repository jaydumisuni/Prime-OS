from pathlib import Path

path = Path("crates/prime-shell/src/visual.rs")
text = path.read_text()
old = '''fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut result = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() && max_chars >= 3 {
        result.truncate(result.len().saturating_sub(3));
        result.push_str("...");
    }
    result
}
'''
new = '''fn truncate(value: &str, max_chars: usize) -> String {
    let character_count = value.chars().count();
    if character_count <= max_chars {
        return value.to_owned();
    }
    if max_chars < 3 {
        return value.chars().take(max_chars).collect();
    }
    let mut result = value.chars().take(max_chars - 3).collect::<String>();
    result.push_str("...");
    result
}
'''
if text.count(old) != 1:
    raise SystemExit("visual truncate anchor changed")
path.write_text(text.replace(old, new, 1))
