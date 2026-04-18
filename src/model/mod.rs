#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Javascript,
    Python,
}

impl Language {
    pub fn all() -> Vec<Language> {
        vec![Language::Javascript, Language::Python]
    }

    pub fn display_name(&self) -> &str {
        match self {
            Language::Javascript => "JavaScript",
            Language::Python => "Python",
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
    // todo: add more languages and dev artifacts
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
        }
    }
}
