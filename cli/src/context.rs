use std::path::Path;
use std::process::Command;

const MAX_INSTRUCTIONS_CHARS: usize = 12_000;

pub struct ProjectContext {
    pub cwd: String,
    pub project_type: String,
    pub git_branch: Option<String>,
    pub git_status: Option<String>,
    pub directory_tree: String,
    /// First matching project instruction file, if any.
    pub instructions: Option<ProjectInstructions>,
}

#[derive(Clone, Debug)]
pub struct ProjectInstructions {
    pub source: String,
    pub body: String,
}

pub fn read_context() -> ProjectContext {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into());

    let project_type = detect_project_type(&cwd);
    let git_branch = git_branch_name();
    let git_status = git_status_summary();
    let directory_tree = directory_tree_summary(&cwd);
    let instructions = load_project_instructions(Path::new(&cwd));

    ProjectContext {
        cwd,
        project_type,
        git_branch,
        git_status,
        directory_tree,
        instructions,
    }
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
    if let Some(ref instructions) = ctx.instructions {
        s.push_str(&format!(
            "\nProject instructions (from {}):\n{}\n",
            instructions.source, instructions.body
        ));
    }
    s
}

/// Load the first matching instruction file under the workspace root.
pub fn load_project_instructions(root: &Path) -> Option<ProjectInstructions> {
    const CANDIDATES: &[&str] = &[
        "AGENTS.md",
        "G0D.md",
        ".g0d/instructions.md",
        ".g0d/AGENTS.md",
        "CLAUDE.md",
    ];
    for relative in CANDIDATES {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let body = truncate_chars(trimmed, MAX_INSTRUCTIONS_CHARS);
        return Some(ProjectInstructions {
            source: relative.to_string(),
            body,
        });
    }
    None
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut out: String = value.chars().take(limit).collect();
    out.push_str("\n... [instructions truncated]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_first_matching_instructions_file() {
        let root = std::env::temp_dir().join(format!(
            "g0d-instructions-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".g0d")).unwrap();
        std::fs::write(root.join("AGENTS.md"), " Prefer small diffs. ").unwrap();
        std::fs::write(root.join("G0D.md"), " ignored ").unwrap();
        let loaded = load_project_instructions(&root).unwrap();
        assert_eq!(loaded.source, "AGENTS.md");
        assert!(loaded.body.contains("small diffs"));
        let _ = std::fs::remove_dir_all(&root);
    }
}

fn detect_project_type(cwd: &str) -> String {
    let path = Path::new(cwd);
    let checks: Vec<(&str, &str)> = vec![
        ("Cargo.toml", "Rust"),
        ("package.json", "Node.js"),
        ("tsconfig.json", "TypeScript"),
        ("go.mod", "Go"),
        ("requirements.txt", "Python"),
        ("pyproject.toml", "Python"),
        ("CMakeLists.txt", "C/C++"),
        ("Makefile", "C/Make"),
        ("pom.xml", "Java Maven"),
        ("build.gradle", "Java Gradle"),
        (".csproj", "C#/.NET"),
        (".sln", "C# Solution"),
        ("Gemfile", "Ruby"),
        ("mix.exs", "Elixir"),
        ("Dockerfile", "Docker"),
        (".git", "Git"),
    ];

    let mut types: Vec<String> = Vec::new();
    for (file, label) in &checks {
        if path.join(file).exists() && !types.iter().any(|t: &String| t.as_str() == *label) {
            types.push(label.to_string());
        }
    }
    if types.is_empty() {
        "Unknown".into()
    } else {
        types.join(", ")
    }
}

fn git_branch_name() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn git_status_summary() -> Option<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return Some("clean".into());
    }

    let (mut m, mut a, mut d, mut u) = (0u32, 0u32, 0u32, 0u32);
    for line in text.lines() {
        if line.len() < 2 {
            continue;
        }
        match &line[..2] {
            " M" | "M " | "MM" => m += 1,
            "A " | "AM" => a += 1,
            "D " | " D" => d += 1,
            "??" => u += 1,
            _ => m += 1,
        }
    }
    let parts: Vec<String> = [
        (m > 0).then(|| format!("{}M", m)),
        (a > 0).then(|| format!("{}A", a)),
        (d > 0).then(|| format!("{}D", d)),
        (u > 0).then(|| format!("{}?", u)),
    ]
    .into_iter()
    .flatten()
    .collect();
    Some(parts.join(" "))
}

fn directory_tree_summary(cwd: &str) -> String {
    let path = Path::new(cwd);
    let Ok(entries) = std::fs::read_dir(path) else {
        return "  (cannot read)\n".into();
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut count = 0u32;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "__pycache__"
        {
            continue;
        }
        count += 1;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(format!("  {}/", name));
        } else {
            files.push(format!("  {}", name));
        }
        if count >= 30 {
            break;
        }
    }
    let mut out = String::new();
    for d in &dirs {
        out.push_str(d);
        out.push('\n');
    }
    for f in &files {
        out.push_str(f);
        out.push('\n');
    }
    if count >= 30 {
        out.push_str("  ...\n");
    }
    out
}
