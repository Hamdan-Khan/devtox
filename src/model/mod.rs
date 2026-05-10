#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Javascript,
    Python,
    Custom,
}

impl Language {
    pub fn all() -> Vec<Language> {
        vec![Language::Javascript, Language::Python, Language::Custom]
    }

    pub fn display_name(&self) -> &str {
        match self {
            Language::Javascript => "JavaScript",
            Language::Python => "Python",
            Language::Custom => "Custom",
        }
    }

    pub fn artifacts(&self) -> Vec<ArtifactKind> {
        match self {
            Language::Javascript => vec![
                ArtifactKind::NodeModules,
                ArtifactKind::NextDir,
                ArtifactKind::DistDir,
                ArtifactKind::DotEnv,
            ],
            Language::Python => vec![ArtifactKind::Venv, ArtifactKind::PycacheDir],
            Language::Custom => vec![ArtifactKind::New],
        }
    }
}

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
            ArtifactKind::New => "+ New",
        }
    }
}
