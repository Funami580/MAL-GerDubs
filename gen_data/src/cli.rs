use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(version)]
/// Generate complete and incomplete dub data with their respective MAL ids
pub(crate) struct Args {
    /// Search for dubs in this language
    #[arg(value_enum, short, long, ignore_case = true, default_value_t = Language::German)]
    pub(crate) language: Language,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Language {
    German,
    English,
    French,
    Italian,
    Spanish,
}

impl Language {
    pub const fn get_anisearch_language(&self) -> &'static str {
        match self {
            Language::German => "de",
            Language::English => "en",
            Language::French => "fr",
            Language::Italian => "it",
            Language::Spanish => "es",
        }
    }
}
