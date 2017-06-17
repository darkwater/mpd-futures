#[derive(Clone, Debug, Default)]
pub struct Song {
    pub file:     Option<String>,
    pub artist:   Option<String>,
    pub album:    Option<String>,
    pub title:    Option<String>,
    pub disc:     Option<u64>,
    pub track:    Option<u64>,
    pub duration: Option<f64>,
    pub pos:      Option<usize>,
    pub id:       Option<usize>,
    pub tags:     Vec<(String, String)>,
}
