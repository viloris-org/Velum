use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

pub fn check(root: &Path) -> Result<(), String> {
    let markdown = markdown_files(root)?;
    let mut errors = Vec::new();
    for path in &markdown {
        validate_file(root, path, &mut errors)?;
    }
    if errors.is_empty() {
        println!(
            "Documentation links valid ({} Markdown files).",
            markdown.len()
        );
        Ok(())
    } else {
        Err(format!(
            "documentation check failed ({}):\n- {}",
            errors.len(),
            errors.join("\n- ")
        ))
    }
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(root).map_err(|error| format!("cannot read {}: {error}", root.display()))?
    {
        let path = entry
            .map_err(|error| format!("cannot read directory entry: {error}"))?
            .path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    for entry in [root.join("docs"), root.join("experiments")] {
        collect_markdown(&entry, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_markdown(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !path.exists() {
        return Ok(());
    }
    for entry in
        fs::read_dir(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        collect_markdown(&entry.path(), files)?;
    }
    Ok(())
}

fn validate_file(root: &Path, source_path: &Path, errors: &mut Vec<String>) -> Result<(), String> {
    let source = fs::read_to_string(source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let mut fenced = false;
    for (line_index, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for destination in link_destinations(line) {
            validate_link(root, source_path, line_index + 1, &destination, errors)?;
        }
    }
    Ok(())
}

fn link_destinations(line: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = line;
    while let Some(label_end) = rest.find("](") {
        let destination = &rest[label_end + 2..];
        let Some(end) = destination.find(')') else {
            break;
        };
        let raw = destination[..end].trim();
        let raw = raw
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            .unwrap_or(raw);
        let target = raw.split_once(" \"").map_or(raw, |(path, _)| path).trim();
        links.push(target.to_owned());
        rest = &destination[end + 1..];
    }
    links
}

fn validate_link(
    root: &Path,
    source: &Path,
    line: usize,
    destination: &str,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    if destination.is_empty()
        || destination.starts_with("http://")
        || destination.starts_with("https://")
        || destination.starts_with("mailto:")
    {
        return Ok(());
    }
    let (path_part, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, fragment)| {
            (path, Some(fragment))
        });
    let target = if path_part.is_empty() {
        source.to_path_buf()
    } else {
        normalize(&source.parent().unwrap_or(root).join(path_part))
    };
    if !target.starts_with(root) {
        errors.push(format!(
            "{}:{line}: link escapes repository: {destination}",
            display(root, source)
        ));
        return Ok(());
    }
    if !target.exists() {
        errors.push(format!(
            "{}:{line}: missing link target: {destination}",
            display(root, source)
        ));
        return Ok(());
    }
    if let Some(fragment) = fragment
        && !fragment.is_empty()
        && target
            .extension()
            .is_some_and(|extension| extension == "md")
    {
        let headings = headings(&target)?;
        if !headings.contains(fragment) {
            errors.push(format!(
                "{}:{line}: missing heading #{fragment} in {}",
                display(root, source),
                display(root, &target)
            ));
        }
    }
    Ok(())
}

fn headings(path: &Path) -> Result<BTreeSet<String>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix('#'))
        .map(|heading| heading.trim_start_matches('#').trim())
        .filter(|heading| !heading.is_empty())
        .map(slug)
        .collect())
}

fn slug(heading: &str) -> String {
    heading
        .chars()
        .filter_map(|character| {
            if character.is_alphanumeric() || character == '-' || character == '_' {
                Some(character.to_ascii_lowercase())
            } else if character.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_links_inside_code_fences() {
        let directory = tempfile::tempdir().expect("tempdir");
        let readme = directory.path().join("README.md");
        fs::write(&readme, "```md\n[missing](missing.md)\n```\n").expect("write fixture");
        let mut errors = Vec::new();
        validate_file(directory.path(), &readme, &mut errors).expect("check fixture");
        assert!(errors.is_empty());
    }

    #[test]
    fn reports_missing_targets() {
        let directory = tempfile::tempdir().expect("tempdir");
        let readme = directory.path().join("README.md");
        fs::write(&readme, "[missing](missing.md)\n").expect("write fixture");
        let mut errors = Vec::new();
        validate_file(directory.path(), &readme, &mut errors).expect("check fixture");
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn validates_markdown_fragments() {
        let directory = tempfile::tempdir().expect("tempdir");
        let readme = directory.path().join("README.md");
        let target = directory.path().join("guide.md");
        fs::write(&readme, "[valid](guide.md#known-heading)\n").expect("write fixture");
        fs::write(&target, "# Known Heading\n").expect("write target");
        let mut errors = Vec::new();
        validate_file(directory.path(), &readme, &mut errors).expect("check fixture");
        assert!(errors.is_empty());
    }
}
