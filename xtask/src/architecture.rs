use cargo_metadata::{Metadata, MetadataCommand, Package};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
struct Contract {
    version: u32,
    governance: Governance,
    modules: BTreeMap<String, Module>,
    #[serde(default)]
    forbidden_dependencies: Vec<ForbiddenDependency>,
    #[serde(default)]
    gates: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
struct Governance {
    governed_paths: Vec<String>,
    #[serde(default)]
    excluded_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Module {
    paths: Vec<String>,
    owner: String,
    #[serde(default)]
    may_depend_on: Vec<String>,
    #[serde(default)]
    public_api: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ForbiddenDependency {
    from: String,
    to: Vec<String>,
}

struct CompiledContract<'a> {
    governed: GlobSet,
    excluded: GlobSet,
    module_paths: BTreeMap<&'a str, GlobSet>,
}

const SUPPORTED_GATES: &[&str] = &[
    "dependency_cycles",
    "forbidden_dependencies",
    "unowned_modules",
    "unowned_governed_files",
];

pub fn check(root: &Path) -> Result<(), String> {
    let contract_path = root.join("docs/architecture-contract.yaml");
    let source = fs::read_to_string(&contract_path)
        .map_err(|error| format!("cannot read {}: {error}", contract_path.display()))?;
    let contract: Contract = serde_yaml_ng::from_str(&source)
        .map_err(|error| format!("cannot parse {}: {error}", contract_path.display()))?;
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;
    let report = evaluate(root, &contract, &metadata)?;
    if report.active_modules.is_empty() {
        println!("Architecture contract valid; all declared runtime modules are inactive.");
    } else {
        println!(
            "Architecture contract valid; active modules: {}.",
            report.active_modules.join(", ")
        );
    }
    Ok(())
}

struct Report {
    active_modules: Vec<String>,
}

fn evaluate(root: &Path, contract: &Contract, metadata: &Metadata) -> Result<Report, String> {
    let compiled = compile(contract)?;
    let mut errors = validate_contract(contract);
    let mut package_modules = BTreeMap::new();
    let mut active_modules = BTreeSet::new();

    for package in &metadata.packages {
        let root_path = package_root(root, package)?;
        if !compiled.governed.is_match(&root_path) || compiled.excluded.is_match(&root_path) {
            continue;
        }
        let matches = matching_modules(&compiled, &format!("{root_path}/Cargo.toml"));
        match matches.as_slice() {
            [] => errors.push(format!(
                "runtime package {} at {root_path} is not owned by a module",
                package.name
            )),
            [module] => {
                package_modules.insert(package.name.as_str(), *module);
                active_modules.insert((*module).to_owned());
            }
            modules => errors.push(format!(
                "runtime package {} at {root_path} matches multiple modules: {}",
                package.name,
                modules.join(", ")
            )),
        }
    }

    validate_governed_files(root, &compiled, &mut errors)?;
    validate_public_apis(root, contract, &active_modules, &mut errors);
    let edges = dependency_edges(metadata, &package_modules);
    validate_edges(contract, &edges, &mut errors);
    validate_cycles(&edges, &mut errors);

    if errors.is_empty() {
        Ok(Report {
            active_modules: active_modules.into_iter().collect(),
        })
    } else {
        Err(format!(
            "architecture contract failed ({}):\n- {}",
            errors.len(),
            errors.join("\n- ")
        ))
    }
}

fn compile(contract: &Contract) -> Result<CompiledContract<'_>, String> {
    let governed = compile_globs(&contract.governance.governed_paths, "governed_paths")?;
    let excluded = compile_globs(&contract.governance.excluded_paths, "excluded_paths")?;
    let mut module_paths = BTreeMap::new();
    for (name, module) in &contract.modules {
        module_paths.insert(name.as_str(), compile_globs(&module.paths, name)?);
    }
    Ok(CompiledContract {
        governed,
        excluded,
        module_paths,
    })
}

fn compile_globs(patterns: &[String], location: &str) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(pattern)
                .map_err(|error| format!("invalid glob {pattern:?} in {location}: {error}"))?,
        );
    }
    builder
        .build()
        .map_err(|error| format!("cannot compile globs in {location}: {error}"))
}

fn validate_contract(contract: &Contract) -> Vec<String> {
    let mut errors = Vec::new();
    if contract.version != 1 {
        errors.push(format!("unsupported contract version {}", contract.version));
    }
    if contract.governance.governed_paths.is_empty() {
        errors.push("governance.governed_paths must not be empty".into());
    }
    for (name, module) in &contract.modules {
        if module.owner.trim().is_empty() {
            errors.push(format!("module {name} has no owner"));
        }
        if module.paths.is_empty() {
            errors.push(format!("module {name} has no paths"));
        }
        for dependency in &module.may_depend_on {
            if !contract.modules.contains_key(dependency) {
                errors.push(format!(
                    "module {name} allows unknown dependency {dependency}"
                ));
            }
        }
    }
    for forbidden in &contract.forbidden_dependencies {
        if !contract.modules.contains_key(&forbidden.from) {
            errors.push(format!(
                "forbidden_dependencies references unknown source {}",
                forbidden.from
            ));
        }
        for target in &forbidden.to {
            if !contract.modules.contains_key(target) {
                errors.push(format!(
                    "forbidden_dependencies from {} references unknown target {target}",
                    forbidden.from
                ));
            }
        }
    }
    for (gate, threshold) in &contract.gates {
        if !SUPPORTED_GATES.contains(&gate.as_str()) {
            errors.push(format!("architecture gate {gate} is unsupported"));
        } else if *threshold != 0 {
            errors.push(format!("structural gate {gate} must have a zero threshold"));
        }
    }
    errors
}

fn matching_modules<'a>(compiled: &'a CompiledContract<'a>, path: &str) -> Vec<&'a str> {
    compiled
        .module_paths
        .iter()
        .filter_map(|(name, globs)| globs.is_match(path).then_some(*name))
        .collect()
}

fn package_root(root: &Path, package: &Package) -> Result<String, String> {
    let manifest = package.manifest_path.as_std_path();
    let package_root = manifest
        .parent()
        .ok_or_else(|| format!("package {} has no manifest parent", package.name))?;
    relative_path(root, package_root)
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("{} is outside workspace {}", path.display(), root.display()))
}

fn validate_governed_files(
    root: &Path,
    compiled: &CompiledContract<'_>,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    for entry in walk_files(root)? {
        let relative = relative_path(root, &entry)?;
        if !compiled.governed.is_match(&relative) || compiled.excluded.is_match(&relative) {
            continue;
        }
        let matches = matching_modules(compiled, &relative);
        if matches.is_empty() {
            errors.push(format!("governed file {relative} is not owned by a module"));
        } else if matches.len() > 1 {
            errors.push(format!(
                "governed file {relative} matches multiple modules: {}",
                matches.join(", ")
            ));
        }
    }
    Ok(())
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
            let path = entry.path();
            if path == root.join(".git") || path == root.join("target") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn validate_public_apis(
    root: &Path,
    contract: &Contract,
    active: &BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    for name in active {
        for api in &contract.modules[name].public_api {
            if !root.join(api).is_file() {
                errors.push(format!("active module {name} is missing public API {api}"));
            }
        }
    }
}

fn dependency_edges<'a>(
    metadata: &'a Metadata,
    package_modules: &BTreeMap<&'a str, &'a str>,
) -> BTreeSet<(&'a str, &'a str)> {
    let mut edges = BTreeSet::new();
    for package in &metadata.packages {
        let Some(&from) = package_modules.get(package.name.as_str()) else {
            continue;
        };
        for dependency in &package.dependencies {
            if dependency.path.is_some()
                && let Some(&to) = package_modules.get(dependency.name.as_str())
                && from != to
            {
                edges.insert((from, to));
            }
        }
    }
    edges
}

fn validate_edges(contract: &Contract, edges: &BTreeSet<(&str, &str)>, errors: &mut Vec<String>) {
    for &(from, to) in edges {
        let allowed = contract.modules[from]
            .may_depend_on
            .iter()
            .any(|candidate| candidate == to);
        if !allowed {
            errors.push(format!("dependency {from} -> {to} is not allowed"));
        }
        if contract
            .forbidden_dependencies
            .iter()
            .any(|rule| rule.from == from && rule.to.iter().any(|target| target == to))
        {
            errors.push(format!("dependency {from} -> {to} is explicitly forbidden"));
        }
    }
}

fn validate_cycles(edges: &BTreeSet<(&str, &str)>, errors: &mut Vec<String>) {
    let nodes: BTreeSet<_> = edges.iter().flat_map(|(from, to)| [*from, *to]).collect();
    for start in nodes {
        let mut visited = BTreeSet::new();
        if reaches(start, start, edges, &mut visited, true) {
            errors.push(format!("dependency cycle includes module {start}"));
        }
    }
}

fn reaches<'a>(
    current: &'a str,
    target: &str,
    edges: &BTreeSet<(&'a str, &'a str)>,
    visited: &mut BTreeSet<&'a str>,
    first: bool,
) -> bool {
    if !first && current == target {
        return true;
    }
    if !visited.insert(current) {
        return false;
    }
    edges
        .iter()
        .filter(|(from, _)| *from == current)
        .any(|(_, next)| reaches(next, target, edges, visited, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(source: &str) -> Contract {
        serde_yaml_ng::from_str(source).expect("valid fixture")
    }

    fn basic_contract(paths: &[&str]) -> Contract {
        contract(&format!(
            r#"
version: 1
governance:
  governed_paths: ["crates/**"]
modules:
  protocol:
    paths: [{}]
    owner: protocol
    public_api: ["crates/protocol/src/lib.rs"]
"#,
            paths
                .iter()
                .map(|path| format!("\"{path}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    #[test]
    fn rejects_unknown_dependency_reference() {
        let source = r#"
version: 1
governance:
  governed_paths: ["crates/**"]
modules:
  protocol:
    paths: ["crates/protocol/**"]
    owner: protocol
    may_depend_on: [missing]
"#;
        let contract = contract(source);
        assert_eq!(
            validate_contract(&contract),
            vec!["module protocol allows unknown dependency missing"]
        );
    }

    #[test]
    fn rejects_declared_but_unsupported_gate() {
        let source = r#"
version: 1
governance:
  governed_paths: ["crates/**"]
modules:
  protocol: { paths: ["crates/protocol/**"], owner: protocol }
gates:
  future_gate: 0
"#;
        let contract = contract(source);
        assert_eq!(
            validate_contract(&contract),
            vec!["architecture gate future_gate is unsupported"]
        );
    }

    #[test]
    fn detects_dependency_cycle() {
        let edges = BTreeSet::from([("a", "b"), ("b", "a")]);
        let mut errors = Vec::new();
        validate_cycles(&edges, &mut errors);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn reports_unowned_governed_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let crates = directory.path().join("crates/unowned/src");
        fs::create_dir_all(&crates).expect("create fixture");
        fs::write(crates.join("lib.rs"), "").expect("write fixture");
        let contract = basic_contract(&["crates/protocol/**"]);
        let compiled = compile(&contract).expect("compile contract");
        let mut errors = Vec::new();
        validate_governed_files(directory.path(), &compiled, &mut errors).expect("walk fixture");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not owned"));
    }

    #[test]
    fn reports_multiple_file_owners() {
        let directory = tempfile::tempdir().expect("tempdir");
        let source = directory.path().join("crates/protocol/src");
        fs::create_dir_all(&source).expect("create fixture");
        fs::write(source.join("lib.rs"), "").expect("write fixture");
        let mut contract = basic_contract(&["crates/protocol/**"]);
        contract.modules.insert(
            "duplicate".into(),
            Module {
                paths: vec!["crates/protocol/**".into()],
                owner: "other".into(),
                may_depend_on: Vec::new(),
                public_api: Vec::new(),
            },
        );
        let compiled = compile(&contract).expect("compile contract");
        let mut errors = Vec::new();
        validate_governed_files(directory.path(), &compiled, &mut errors).expect("walk fixture");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("multiple modules"));
    }

    #[test]
    fn rejects_disallowed_and_forbidden_edge() {
        let source = r#"
version: 1
governance:
  governed_paths: ["crates/**"]
modules:
  protocol: { paths: ["crates/protocol/**"], owner: protocol }
  session: { paths: ["crates/session/**"], owner: session }
forbidden_dependencies:
  - from: protocol
    to: [session]
"#;
        let contract = contract(source);
        let mut errors = Vec::new();
        validate_edges(
            &contract,
            &BTreeSet::from([("protocol", "session")]),
            &mut errors,
        );
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn active_module_requires_public_api() {
        let directory = tempfile::tempdir().expect("tempdir");
        let contract = basic_contract(&["crates/protocol/**"]);
        let active = BTreeSet::from(["protocol".to_owned()]);
        let mut errors = Vec::new();
        validate_public_apis(directory.path(), &contract, &active, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("missing public API"));
    }
}
