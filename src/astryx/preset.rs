#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemePreset {
    #[default]
    Neutral,
    Butter,
    Chocolate,
    Gothic,
    Matcha,
    Stone,
    Y2k,
}

impl ThemePreset {
    pub const ALL: [Self; 7] = [
        Self::Neutral, Self::Butter, Self::Chocolate, Self::Gothic, Self::Matcha, Self::Stone, Self::Y2k,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Butter => "butter",
            Self::Chocolate => "chocolate",
            Self::Gothic => "gothic",
            Self::Matcha => "matcha",
            Self::Stone => "stone",
            Self::Y2k => "y2k",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Neutral => "Neutral",
            Self::Butter => "Butter",
            Self::Chocolate => "Chocolate",
            Self::Gothic => "Gothic",
            Self::Matcha => "Matcha",
            Self::Stone => "Stone",
            Self::Y2k => "Y2K",
        }
    }
}
