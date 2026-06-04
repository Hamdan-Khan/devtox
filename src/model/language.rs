use crate::model::artifact::ArtifactKind;

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Javascript,
    Python,
    Rust,
    Custom,
}

impl Language {
    pub fn all() -> Vec<Language> {
        vec![
            Language::Javascript,
            Language::Python,
            Language::Rust,
            Language::Custom,
        ]
    }

    pub fn display_name(&self) -> &str {
        match self {
            Language::Javascript => "JavaScript",
            Language::Python => "Python",
            Language::Rust => "Rust",
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
            Language::Rust => vec![ArtifactKind::Target],
        }
    }
}
