use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};
use yahoo_finance_api as yahoo;

/// Rate limiting configuration for stock price queries
const REQUESTS_PER_EVAL: usize = 3; // Max 3 stock queries per TCL eval
const REQUESTS_PER_MINUTE_PER_USER: usize = 10; // 10 per minute per user
const REQUESTS_PER_MINUTE_GLOBAL: usize = 30; // 30 per minute globally
const REQUEST_INTERVAL_SECS: u64 = 60;
const CACHE_TTL_SECS: u64 = 60; // Cache results for 1 minute

#[derive(Debug, Clone)]
struct RequestRecord {
    timestamp: u64,
    eval_count: u64,
    user: String,
}

/// Cached stock quote data
#[derive(Debug, Clone)]
pub(crate) struct CachedQuote {
    pub timestamp: u64,
    pub symbol: String,
    pub price: f64,
    pub change_percent: f64,
    pub volume: u64,
}

/// Rate limiter for stock price queries
#[derive(Debug)]
pub struct StockRateLimiter {
    requests: Vec<RequestRecord>,
    current_user: String,
    current_eval_count: u64,
    cache: HashMap<String, CachedQuote>,
}

impl StockRateLimiter {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            current_user: String::new(),
            current_eval_count: 0,
            cache: HashMap::new(),
        }
    }

    pub fn set_context(&mut self, user: String, eval_count: u64) {
        self.current_user = user;
        self.current_eval_count = eval_count;
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn cleanup_old_requests(&mut self) {
        let now = Self::now();
        let threshold = now.saturating_sub(REQUEST_INTERVAL_SECS);
        self.requests.retain(|r| r.timestamp >= threshold);
    }

    fn cleanup_cache(&mut self) {
        let now = Self::now();
        let threshold = now.saturating_sub(CACHE_TTL_SECS);
        self.cache.retain(|_, quote| quote.timestamp >= threshold);
    }

    pub(crate) fn get_cached(&mut self, symbol: &str) -> Option<CachedQuote> {
        self.cleanup_cache();
        self.cache.get(symbol).cloned()
    }

    pub(crate) fn put_cache(&mut self, quote: CachedQuote) {
        self.cache.insert(quote.symbol.clone(), quote);
    }

    pub fn check_and_record(&mut self) -> Result<()> {
        if self.current_user.is_empty() {
            return Err(anyhow!("Stock rate limiter context not set"));
        }

        self.cleanup_old_requests();

        let now = Self::now();

        // Check per-eval limit
        let eval_count = self
            .requests
            .iter()
            .filter(|r| r.eval_count == self.current_eval_count)
            .count();

        if eval_count >= REQUESTS_PER_EVAL {
            return Err(anyhow!(
                "Too many stock queries in this eval (max {} per eval)",
                REQUESTS_PER_EVAL
            ));
        }

        // Check per-user limit
        let user_count = self
            .requests
            .iter()
            .filter(|r| r.user == self.current_user)
            .count();

        if user_count >= REQUESTS_PER_MINUTE_PER_USER {
            return Err(anyhow!(
                "Too many stock queries (max {} per minute per user)",
                REQUESTS_PER_MINUTE_PER_USER
            ));
        }

        // Check global limit
        let global_count = self.requests.len();
        if global_count >= REQUESTS_PER_MINUTE_GLOBAL {
            return Err(anyhow!(
                "Too many stock queries globally (max {} per minute)",
                REQUESTS_PER_MINUTE_GLOBAL
            ));
        }

        // Record this request
        self.requests.push(RequestRecord {
            timestamp: now,
            eval_count: self.current_eval_count,
            user: self.current_user.clone(),
        });

        debug!(
            "Stock request recorded: user={}, eval={}, count_in_eval={}, count_per_user={}, global={}",
            self.current_user,
            self.current_eval_count,
            eval_count + 1,
            user_count + 1,
            global_count + 1
        );

        Ok(())
    }
}

/// Global stock client with rate limiting
pub struct StockClient {
    rate_limiter: Arc<Mutex<StockRateLimiter>>,
    provider: yahoo::YahooConnector,
}

impl StockClient {
    pub fn new() -> Self {
        // Use builder with explicit User-Agent to avoid Yahoo Finance rate limiting
        let provider = yahoo::YahooConnector::builder()
            .build()
            .unwrap_or_else(|_| yahoo::YahooConnector::new().unwrap());

        Self {
            rate_limiter: Arc::new(Mutex::new(StockRateLimiter::new())),
            provider,
        }
    }

    pub fn set_context(&self, user: String, eval_count: u64) {
        if let Ok(mut limiter) = self.rate_limiter.lock() {
            limiter.set_context(user, eval_count);
        }
    }

    /// Get current price for a stock symbol
    pub(crate) fn get_quote(&self, symbol: &str) -> Result<CachedQuote> {
        // Check cache first
        if let Ok(mut limiter) = self.rate_limiter.lock() {
            if let Some(cached) = limiter.get_cached(symbol) {
                debug!("Cache hit for symbol: {}", symbol);
                return Ok(cached);
            }
        }

        // Check rate limit
        if let Ok(mut limiter) = self.rate_limiter.lock() {
            limiter.check_and_record()?;
        } else {
            return Err(anyhow!("Failed to acquire rate limiter lock"));
        }

        debug!("Fetching quote for symbol: {}", symbol);

        // Fetch from Yahoo Finance (create a new single-threaded runtime)
        // We're in a separate OS thread (not Tokio context), so we need our own runtime
        // Using current_thread to avoid spawning additional threads
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow!("Failed to create Tokio runtime: {}", e))?;

        let quote = runtime.block_on(async {
            self.provider.get_latest_quotes(symbol, "1d").await
        })
        .map_err(|e| {
            warn!("Failed to fetch quote for {}: {:?}", symbol, e);
            anyhow!("Failed to fetch stock data: {}", e)
        })?;

        // Extract the most recent quote
        let last_quote = quote
            .last_quote()
            .map_err(|e| anyhow!("No quote data available: {}", e))?;

        // Calculate change percent
        let price = last_quote.close;
        let quotes_vec = quote.quotes().map_err(|e| anyhow!("Failed to get quotes: {}", e))?;
        let change_percent = if quotes_vec.len() >= 2 {
            let prev_close = &quotes_vec[quotes_vec.len() - 2];
            ((price - prev_close.close) / prev_close.close) * 100.0
        } else {
            0.0
        };

        let cached_quote = CachedQuote {
            timestamp: StockRateLimiter::now(),
            symbol: symbol.to_uppercase(),
            price,
            change_percent,
            volume: last_quote.volume,
        };

        // Store in cache
        if let Ok(mut limiter) = self.rate_limiter.lock() {
            limiter.put_cache(cached_quote.clone());
        }

        Ok(cached_quote)
    }

    pub fn get_formatted_quote(&self, symbol: &str) -> Result<String> {
        let quote = self.get_quote(symbol)?;
        let sign = if quote.change_percent >= 0.0 { "+" } else { "" };
        Ok(format!(
            "{}: ${:.2} ({}{}%)",
            quote.symbol,
            quote.price,
            sign,
            format!("{:.2}", quote.change_percent)
        ))
    }

    /// Get historical quotes for charting with configurable interval
    ///
    /// # Arguments
    /// * `symbol` - Stock symbol (e.g., "AAPL")
    /// * `days` - Number of days to fetch
    /// * `interval` - Optional interval ("1m", "5m", "15m", "30m", "1h", "1d", "1wk", "1mo")
    ///                If None, uses smart defaults based on the time range
    pub(crate) fn get_historical_quotes(&self, symbol: &str, days: usize, interval: Option<&str>) -> Result<Vec<(i64, f64)>> {
        // Check rate limit
        if let Ok(mut limiter) = self.rate_limiter.lock() {
            limiter.check_and_record()?;
        } else {
            return Err(anyhow!("Failed to acquire rate limiter lock"));
        }

        // Smart defaults based on time range
        let interval_str = match interval {
            Some(i) => i,
            None => {
                if days == 1 {
                    "5m"  // 1 day: 5-minute intervals
                } else if days <= 7 {
                    "1h"  // 2-7 days: hourly intervals
                } else if days <= 60 {
                    "1d"  // 8-60 days: daily intervals
                } else {
                    "1wk" // 60+ days: weekly intervals
                }
            }
        };

        debug!("Fetching historical quotes for symbol: {} ({} days, {} interval)", symbol, days, interval_str);

        // Calculate time range for the query
        use yahoo_finance_api::time::{OffsetDateTime, Duration};
        let now = OffsetDateTime::now_utc();
        let days_duration = Duration::days(days as i64);
        let start_time = now - days_duration;

        // Fetch from Yahoo Finance with interval
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow!("Failed to create Tokio runtime: {}", e))?;

        let response = runtime.block_on(async {
            self.provider.get_quote_history_interval(symbol, start_time, now, interval_str).await
        })
        .map_err(|e| {
            warn!("Failed to fetch historical quotes for {}: {:?}", symbol, e);
            anyhow!("Failed to fetch historical stock data: {}", e)
        })?;

        let quotes = response.quotes().map_err(|e| anyhow!("Failed to get quotes: {}", e))?;

        // Return (timestamp, close_price) pairs
        let result: Vec<(i64, f64)> = quotes
            .iter()
            .map(|q| (q.timestamp, q.close))
            .collect();

        Ok(result)
    }
}

// Global stock client instance
lazy_static::lazy_static! {
    static ref STOCK_CLIENT: StockClient = StockClient::new();
}

/// Generate TCL wrapper commands that call back into Rust
/// NOTE: Currently unused - stock commands are intercepted in Rust before TCL evaluation.
/// Kept for reference in case we want to switch to TCL-based dispatch.
#[allow(dead_code)]
pub fn stock_commands() -> &'static str {
    r#"
# Stock price tracking commands (Rust-backed)
namespace eval stock {
    # These are placeholder procs - actual implementation handled by Rust
    # The Rust side intercepts these calls in tcl_thread.rs

    proc quote {symbol} {
        error "stock::quote not properly initialized"
    }

    proc price {symbol} {
        error "stock::price not properly initialized"
    }

    proc detail {symbol} {
        error "stock::detail not properly initialized"
    }
}
"#
}

/// Set context for rate limiting (call before each eval)
pub fn set_stock_context(user: String, eval_count: u64) {
    STOCK_CLIENT.set_context(user, eval_count);
}

/// Handle stock command (called from tcl_thread.rs)
pub fn handle_stock_command(command: &str) -> Result<String> {
    let parts: Vec<&str> = command.trim().split_whitespace().collect();

    if parts.len() < 2 {
        return Err(anyhow!("Usage: stock::command <symbol> [days]"));
    }

    let cmd = parts[0];
    let symbol = parts[1];

    match cmd {
        "stock::quote" => STOCK_CLIENT.get_formatted_quote(symbol),
        "stock::price" => {
            let quote = STOCK_CLIENT.get_quote(symbol)?;
            Ok(format!("{:.2}", quote.price))
        }
        "stock::detail" => {
            let quote = STOCK_CLIENT.get_quote(symbol)?;
            let sign = if quote.change_percent >= 0.0 { "+" } else { "" };
            Ok(format!(
                "symbol {{{}}} price {:.2} change {{{}{}%}} volume {}",
                quote.symbol,
                quote.price,
                sign,
                format!("{:.2}", quote.change_percent),
                quote.volume
            ))
        }
        "stock::history" => {
            // Get optional days parameter (default to 7)
            let days = if parts.len() > 2 {
                parts[2].parse::<usize>().unwrap_or(7)
            } else {
                7
            };

            // Get optional interval parameter (uses smart defaults if not provided)
            let interval = if parts.len() > 3 {
                Some(parts[3])
            } else {
                None
            };

            let history = STOCK_CLIENT.get_historical_quotes(symbol, days, interval)?;

            // Format as TCL list: {timestamp1 price1} {timestamp2 price2} ...
            let formatted: Vec<String> = history
                .iter()
                .map(|(ts, price)| format!("{{{} {:.2}}}", ts, price))
                .collect();

            Ok(formatted.join(" "))
        }
        _ => Err(anyhow!("Unknown stock command: {}", cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_per_eval() {
        let mut limiter = StockRateLimiter::new();
        limiter.set_context("testuser".to_string(), 1);

        // Should allow REQUESTS_PER_EVAL requests
        for _ in 0..REQUESTS_PER_EVAL {
            assert!(limiter.check_and_record().is_ok());
        }

        // Next should fail
        assert!(limiter.check_and_record().is_err());

        // New eval should reset
        limiter.set_context("testuser".to_string(), 2);
        assert!(limiter.check_and_record().is_ok());
    }

    #[test]
    fn test_cache() {
        let mut limiter = StockRateLimiter::new();

        let quote = CachedQuote {
            timestamp: StockRateLimiter::now(),
            symbol: "AAPL".to_string(),
            price: 150.0,
            change_percent: 2.5,
            volume: 1000000,
        };

        limiter.put_cache(quote.clone());

        let cached = limiter.get_cached("AAPL");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().price, 150.0);
    }
}
