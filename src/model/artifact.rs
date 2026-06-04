#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactKind {
    // js / ts
    NodeModules,
    NextDir,
    DistDir,
    DotEnv,
    // python
    Venv,
    PycacheDir,
    // rust
    Target,
    // Custom
    New, // todo: add more languages and dev artifacts
}

impl ArtifactKind {
    pub fn display_name(&self) -> &str {
        match self {
            ArtifactKind::NodeModules => "node_modules",
            ArtifactKind::NextDir => ".next",
            ArtifactKind::DistDir => "dist",
            ArtifactKind::DotEnv => ".env",
            ArtifactKind::Venv => ".venv",
            ArtifactKind::PycacheDir => "__pycache__",
            ArtifactKind::Target => "target",
            ArtifactKind::New => "+ New",
        }
    }
}
