use chrono::{DateTime, Local};

/// A single gauge to display, e.g. "Session (5h): 34%".
pub struct Metric {
    pub label: String,
    /// 0..=100 (may exceed 100 if the provider reports overage).
    pub percent: f64,
    pub resets_at: Option<DateTime<Local>>,
    /// The metric shown in the menu bar title. If several are emphasized,
    /// the highest percentage wins.
    pub emphasized: bool,
}

/// Why a fetch produced no metrics. Rate limiting is separate from real
/// failures so the UI can keep the last good reading instead of showing ⚠.
pub enum FetchError {
    /// 429 that persisted through retries — data is stale, not wrong.
    RateLimited,
    Failed(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::RateLimited => write!(f, "rate limited by API"),
            FetchError::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

/// Anything that can produce metrics: plan usage today, local token counts,
/// API spend, CI status, ... Add a new source by implementing this trait and
/// registering it in `main::sources()`.
pub trait MetricSource: Send {
    /// Section heading shown in the dropdown menu.
    fn name(&self) -> &str;
    fn fetch(&self) -> Result<Vec<Metric>, FetchError>;
}

/// Render a text progress bar, e.g. `▰▰▰▱▱▱▱▱▱▱` for 30%.
/// (Shade blocks like ░/▓ look identical at menu bar size — keep glyphs
/// that stay visually distinct when small.)
pub fn bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round().clamp(0.0, width as f64) as usize;
    let mut s = String::with_capacity(width * 3);
    for i in 0..width {
        s.push(if i < filled { '▰' } else { '▱' });
    }
    s
}
