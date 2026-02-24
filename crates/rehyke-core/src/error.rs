#[derive(Debug, thiserror::Error)]
pub enum RehykeError {
    #[error("HTTP error for {url}: {status}")]
    HttpError { url: String, status: u16 },

    #[error("Connection timeout for {url}")]
    Timeout { url: String },

    #[error("DNS resolution failed for {domain}")]
    DnsError { domain: String },

    #[error("TLS/SSL error for {url}: {message}")]
    TlsError { url: String, message: String },

    #[error("JavaScript rendering failed for {url}: {message}")]
    RenderError { url: String, message: String },

    #[error("Browser launch failed: {message}")]
    BrowserError { message: String },

    #[error("Parse error for {url}: {message}")]
    ParseError { url: String, message: String },

    #[error("Proxy error: {message}")]
    ProxyError { message: String },

    #[error("Rate limited by {domain}")]
    RateLimited { domain: String },

    #[error("Max pages limit reached: {limit}")]
    MaxPagesReached { limit: usize },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, RehykeError>;
