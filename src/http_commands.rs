use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

const REQUESTS_PER_EVAL: usize = 5;
const REQUESTS_PER_MINUTE: usize = 25;
const REQUEST_INTERVAL_SECS: u64 = 60;
const POST_BODY_LIMIT: usize = 150_000;
const TRANSFER_LIMIT: usize = 150_000;
const TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone)]
struct RequestRecord {
    timestamp: u64,
    eval_count: u64,
}

#[derive(Debug)]
pub struct HttpRateLimiter {
    /// Requests per channel
    requests: HashMap<String, Vec<RequestRecord>>,
    current_channel: String,
    current_eval_count: u64,
}

impl HttpRateLimiter {
    pub fn new() -> Self {
        Self {
            requests: HashMap::new(),
            current_channel: String::new(),
            current_eval_count: 0,
        }
    }

    pub fn set_context(&mut self, channel: String, eval_count: u64) {
        self.current_channel = channel;
        self.current_eval_count = eval_count;
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub fn check_and_record(&mut self) -> Result<()> {
        if self.current_channel.is_empty() {
            return Err(anyhow!("HTTP rate limiter context not set"));
        }

        let now = Self::now();
        let threshold = now.saturating_sub(REQUEST_INTERVAL_SECS);

        // Get or create request history for this channel
        let history = self
            .requests
            .entry(self.current_channel.clone())
            .or_insert_with(Vec::new);

        // Clean up old requests (older than 60 seconds)
        history.retain(|r| r.timestamp >= threshold);

        // Check limits
        let eval_count = history
            .iter()
            .filter(|r| r.eval_count == self.current_eval_count)
            .count();

        if eval_count >= REQUESTS_PER_EVAL {
            return Err(anyhow!(
                "too many HTTP requests in this eval (max {} requests)",
                REQUESTS_PER_EVAL
            ));
        }

        let minute_count = history.len();
        if minute_count >= REQUESTS_PER_MINUTE {
            return Err(anyhow!(
                "too many HTTP requests (max {} requests in {} seconds)",
                REQUESTS_PER_MINUTE,
                REQUEST_INTERVAL_SECS
            ));
        }

        // Record this request
        history.push(RequestRecord {
            timestamp: now,
            eval_count: self.current_eval_count,
        });

        debug!(
            "HTTP request recorded: channel={}, eval={}, count_in_eval={}, count_in_minute={}",
            self.current_channel,
            self.current_eval_count,
            eval_count + 1,
            minute_count + 1
        );

        Ok(())
    }
}

pub struct HttpClient {
    rate_limiter: HttpRateLimiter,
    agent: ureq::Agent,
}

impl HttpClient {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (compatible; Slopdrop IRC Bot)")
            .build();

        Self {
            rate_limiter: HttpRateLimiter::new(),
            agent,
        }
    }

    pub fn set_context(&mut self, channel: String, eval_count: u64) {
        self.rate_limiter.set_context(channel, eval_count);
    }

    /// Perform HTTP GET request
    /// Returns TCL list: [status_code, headers_dict, body]
    pub fn get(&mut self, url: &str) -> Result<String> {
        self.rate_limiter.check_and_record()?;

        debug!("HTTP GET: {}", url);

        let response = self.agent.get(url).call().map_err(|e| {
            warn!("HTTP GET failed: {}", e);
            anyhow!("HTTP request failed: {}", e)
        })?;

        let status = response.status();
        let headers = self.format_headers(&response);
        let body = self.read_body_limited(response)?;

        Ok(self.format_response(status, &headers, &body))
    }

    /// Perform HTTP POST request
    /// Returns TCL list: [status_code, headers_dict, body]
    pub fn post(&mut self, url: &str, body: &str) -> Result<String> {
        self.rate_limiter.check_and_record()?;

        if body.len() > POST_BODY_LIMIT {
            return Err(anyhow!(
                "post body exceeds {} bytes",
                POST_BODY_LIMIT
            ));
        }

        debug!("HTTP POST: {} (body size: {})", url, body.len());

        let response = self
            .agent
            .post(url)
            .send_string(body)
            .map_err(|e| {
                warn!("HTTP POST failed: {}", e);
                anyhow!("HTTP request failed: {}", e)
            })?;

        let status = response.status();
        let headers = self.format_headers(&response);
        let response_body = self.read_body_limited(response)?;

        Ok(self.format_response(status, &headers, &response_body))
    }

    /// Perform HTTP HEAD request
    /// Returns TCL dict: {header value ...}
    pub fn head(&mut self, url: &str) -> Result<String> {
        self.rate_limiter.check_and_record()?;

        debug!("HTTP HEAD: {}", url);

        let response = self.agent.head(url).call().map_err(|e| {
            warn!("HTTP HEAD failed: {}", e);
            anyhow!("HTTP request failed: {}", e)
        })?;

        let headers = self.format_headers(&response);
        Ok(headers)
    }

    fn read_body_limited(&self, response: ureq::Response) -> Result<String> {
        use std::io::Read;

        let mut reader = response.into_reader();
        let mut buffer = Vec::new();
        let mut limited_reader = reader.take(TRANSFER_LIMIT as u64);

        limited_reader.read_to_end(&mut buffer).map_err(|e| {
            warn!("Failed to read response body: {}", e);
            anyhow!("Failed to read response: {}", e)
        })?;

        // Check if we hit the limit
        if buffer.len() >= TRANSFER_LIMIT {
            return Err(anyhow!(
                "transfer exceeded {} bytes",
                TRANSFER_LIMIT
            ));
        }

        String::from_utf8(buffer)
            .or_else(|e| {
                // If not valid UTF-8, try to recover what we can
                warn!("Response body is not valid UTF-8: {}", e);
                Ok(String::from_utf8_lossy(&e.into_bytes()).to_string())
            })
    }

    fn format_headers(&self, response: &ureq::Response) -> String {
        let mut dict_items = Vec::new();

        for name in response.headers_names() {
            if let Some(value) = response.header(&name) {
                // Escape TCL special characters
                let escaped_name = self.tcl_escape(&name);
                let escaped_value = self.tcl_escape(value);
                dict_items.push(format!("{} {}", escaped_name, escaped_value));
            }
        }

        dict_items.join(" ")
    }

    fn format_response(&self, status: u16, headers: &str, body: &str) -> String {
        let escaped_body = self.tcl_escape(body);
        format!("{} {{{}}} {}", status, headers, escaped_body)
    }

    fn tcl_escape(&self, s: &str) -> String {
        // TCL list/dict escaping: wrap in braces if contains special chars
        if s.contains(' ')
            || s.contains('{')
            || s.contains('}')
            || s.contains('[')
            || s.contains(']')
            || s.contains('$')
            || s.contains('\\')
            || s.contains('"')
            || s.contains('\n')
        {
            // Use brace quoting
            let mut result = String::from("{");
            for ch in s.chars() {
                if ch == '{' || ch == '}' {
                    result.push('\\');
                }
                result.push(ch);
            }
            result.push('}');
            result
        } else {
            s.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_per_eval() {
        let mut limiter = HttpRateLimiter::new();
        limiter.set_context("##test".to_string(), 1);

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(limiter.check_and_record().is_ok());
        }

        // 6th should fail
        assert!(limiter.check_and_record().is_err());

        // New eval should reset
        limiter.set_context("##test".to_string(), 2);
        assert!(limiter.check_and_record().is_ok());
    }

    #[test]
    fn test_tcl_escape() {
        let client = HttpClient::new();
        assert_eq!(client.tcl_escape("simple"), "simple");
        assert_eq!(client.tcl_escape("hello world"), "{hello world}");
        assert_eq!(client.tcl_escape("test{123}"), "{test\\{123\\}}");
    }
}
