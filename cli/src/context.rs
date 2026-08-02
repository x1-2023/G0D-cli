use std::path::Path;
use std::process::Command;

pub struct ProjectContext {
    pub cwd: String,
    pub project_type: String,
    pub git_branch: Option<String>,
    pub git_status: Option<String>,
    pub directory_tree: String,
}

pub fn read_context() -> ProjectContext {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into());

    let project_type = detect_project_type(&cwd);
    let git_branch = git_branch_name();
    let git_status = git_status_summary();
    let directory_tree = directory_tree_summary(&cwd);

    ProjectContext { cwd, project_type, git_branch, git_status, directory_tree }
}

pub fn context_summary(ctx: &ProjectContext) -> String {
    let mut s = String::new();
    s.push_str(&format!("Working directory: {}\n", ctx.cwd));
    s.push_str(&format!("Project type: {}\n", ctx.project_type));
    if let Some(ref branch) = ctx.git_branch {
        s.push_str(&format!("Git branch: {}\n", branch));
    }
    if let Some(ref status) = ctx.git_status {
        s.push_str(&format!("Git status: {}\n", status));
    }
    s.push_str(&format!("Directory structure:\n{}\n", ctx.directory_tree));
    s
}

fn detect_project_type(cwd: &str) -> String {
    let path = Path::new(cwd);

    let checks: Vec<(&str, &str)> = vec![
        ("Cargo.toml", "Rust"),
        ("package.json", "Node.js/JavaScript"),
        ("tsconfig.json", "TypeScript"),
        ("go.mod", "Go"),
        ("requirements.txt", "Python"),
        ("pyproject.toml", "Python"),
        ("setup.py", "Python"),
        ("Pipfile", "Python"),
        ("CMakeLists.txt", "C/C++"),
        ("Makefile", "C/C++ Make"),
        ("pom.xml", "Java Maven"),
        ("build.gradle", "Java Gradle"),
        (".csproj", "C#/.NET"),
        (".sln", "C#/.NET Solution"),
        ("Cargo.lock", "Rust"),
        ("Gemfile", "Ruby"),
        ("mix.exs", "Elixir"),
        ("Dockerfile", "Docker"),
        (".git", "Git repository"),
    ];

    let mut types: Vec<String> = Vec::new();
    for (file, label) in &checks {
        if path.join(file).exists() {
            if !types.iter().any(|t| t == *label) { types.push(label.to_string()); }
        }
    }

    if types.is_empty() { "Unknown".into() } else { types.join(", ") }
}

fn git_branch_name() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn git_status_summary() -> Option<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output().ok()?;

    if !output.status.success() { return None; }

    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() { return Some("clean".into()); }

    let mut modified = 0u32;
    let mut added = 0u32;
    let mut deleted = 0u32;
    let mut untracked = 0u32;

    for line in text.lines() {
        if line.len() < 2 { continue; }
        match &line[..2] {
            " M" | "M " | "MM" => modified += 1,
            "A " | "AM" => added += 1,
            "D " | " D" => deleted += 1,
            "??" => untracked += 1,
            _ => modified += 1,
        }
    }

    let mut parts = Vec::new();
    if modified > 0 { parts.push(format!("{} modified", modified)); }
    if added > 0 { parts.push(format!("{} added", added)); }
    if deleted > 0 { parts.push(format!("{} deleted", deleted)); }
    if untracked > 0 { parts.push(format!("{} untracked", untracked)); }

    Some(parts.join(", "))
}

fn directory_tree_summary(cwd: &str) -> String {
    let path = Path::new(cwd);

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            let mut count = 0u32;

            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" || name == "__pycache__" {
                    continue;
                }
                count += 1;
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    dirs.push(format!("  {}/", name));
                } else {
                    files.push(format!("  {}", name));
                }
                if count >= 30 { break; }
            }

            let mut out = String::new();
            for d in &dirs { out.push_str(d); out.push('\n'); }
            for f in &files { out.push_str(f); out.push('\n'); }
            if count >= 30 { out.push_str("  ... (truncated)\n"); }
            out
        }
        Err(_) => "  (cannot read directory)\n".into(),
    }
}
