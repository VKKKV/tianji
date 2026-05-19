use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{AlertTier, TianJiError};

pub const TELEGRAM_TEXT_LIMIT: usize = 4096;
pub const DISCORD_CONTENT_LIMIT: usize = 2000;
const HTTP_BODY_SNIPPET_LIMIT: usize = 256;

#[derive(Clone)]
pub struct AlertDispatchConfig {
    pub dry_run: bool,
    pub channels: Vec<AlertChannelConfig>,
    pub policy: AlertDispatchPolicy,
}

impl fmt::Debug for AlertDispatchConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AlertDispatchConfig")
            .field("dry_run", &self.dry_run)
            .field("channels", &self.channels)
            .field("policy", &self.policy)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlertDispatchPolicy {
    pub flash: Vec<String>,
    pub priority: Vec<String>,
    pub routine: Vec<String>,
}

impl AlertDispatchPolicy {
    pub fn channels_for(&self, tier: &AlertTier) -> &[String] {
        match tier {
            AlertTier::Flash => &self.flash,
            AlertTier::Priority => &self.priority,
            AlertTier::Routine => &self.routine,
        }
    }
}

#[derive(Clone)]
pub enum AlertChannelConfig {
    Telegram(TelegramChannelConfig),
    Discord(DiscordChannelConfig),
    Webhook(WebhookChannelConfig),
}

impl AlertChannelConfig {
    pub fn name(&self) -> &str {
        match self {
            Self::Telegram(config) => &config.name,
            Self::Discord(config) => &config.name,
            Self::Webhook(config) => &config.name,
        }
    }

    pub fn kind(&self) -> AlertChannelKind {
        match self {
            Self::Telegram(_) => AlertChannelKind::Telegram,
            Self::Discord(_) => AlertChannelKind::Discord,
            Self::Webhook(_) => AlertChannelKind::Webhook,
        }
    }
}

impl fmt::Debug for AlertChannelConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Telegram(config) => config.fmt(formatter),
            Self::Discord(config) => config.fmt(formatter),
            Self::Webhook(config) => config.fmt(formatter),
        }
    }
}

#[derive(Clone)]
pub struct TelegramChannelConfig {
    pub name: String,
    pub bot_token: String,
    pub chat_id: String,
    pub message_thread_id: Option<i64>,
    pub api_base_url: Option<String>,
}

impl fmt::Debug for TelegramChannelConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TelegramChannelConfig")
            .field("name", &self.name)
            .field("bot_token", &"<redacted>")
            .field("chat_id", &self.chat_id)
            .field("message_thread_id", &self.message_thread_id)
            .field(
                "api_base_url",
                &self.api_base_url.as_ref().map(|url| redact_url(url)),
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct DiscordChannelConfig {
    pub name: String,
    pub webhook_url: String,
}

impl fmt::Debug for DiscordChannelConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscordChannelConfig")
            .field("name", &self.name)
            .field("webhook_url", &redact_url(&self.webhook_url))
            .finish()
    }
}

#[derive(Clone)]
pub struct WebhookChannelConfig {
    pub name: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

impl fmt::Debug for WebhookChannelConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebhookChannelConfig")
            .field("name", &self.name)
            .field("url", &redact_url(&self.url))
            .field("headers", &redacted_headers(&self.headers))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertChannelKind {
    Telegram,
    Discord,
    Webhook,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertMessage {
    pub tier: AlertTier,
    pub title: String,
    pub summary: String,
    pub body: String,
}

impl AlertMessage {
    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        if !self.title.trim().is_empty() {
            parts.push(self.title.trim());
        }
        if !self.summary.trim().is_empty() {
            parts.push(self.summary.trim());
        }
        if !self.body.trim().is_empty() {
            parts.push(self.body.trim());
        }
        parts.join("\n\n")
    }
}

#[derive(Clone, Debug)]
pub struct AlertDispatcher {
    config: AlertDispatchConfig,
    client: reqwest::Client,
}

impl AlertDispatcher {
    pub fn new(config: AlertDispatchConfig) -> Self {
        Self::with_client(config, default_http_client())
    }

    pub fn with_client(config: AlertDispatchConfig, client: reqwest::Client) -> Self {
        Self { config, client }
    }

    pub async fn dispatch(&self, message: &AlertMessage) -> Result<DispatchReport, TianJiError> {
        let channel_map: HashMap<&str, &AlertChannelConfig> = self
            .config
            .channels
            .iter()
            .map(|channel| (channel.name(), channel))
            .collect();
        let mut deliveries = Vec::new();

        for channel_name in self.config.policy.channels_for(&message.tier) {
            let channel = channel_map.get(channel_name.as_str()).ok_or_else(|| {
                TianJiError::Usage(format!(
                    "alert dispatch policy references unknown channel '{channel_name}'"
                ))
            })?;

            if self.config.dry_run {
                deliveries.push(self.planned_delivery(channel, message));
                continue;
            }

            deliveries.push(self.send_delivery(channel, message).await?);
        }

        Ok(DispatchReport {
            dry_run: self.config.dry_run,
            deliveries,
        })
    }

    fn planned_delivery(
        &self,
        channel: &AlertChannelConfig,
        message: &AlertMessage,
    ) -> DeliveryReport {
        let chunks = chunk_count(channel, message);
        DeliveryReport {
            channel_name: channel.name().to_string(),
            channel_kind: channel.kind(),
            status: DeliveryStatus::Planned,
            chunks,
            endpoint: redacted_endpoint(channel),
        }
    }

    async fn send_delivery(
        &self,
        channel: &AlertChannelConfig,
        message: &AlertMessage,
    ) -> Result<DeliveryReport, TianJiError> {
        match channel {
            AlertChannelConfig::Telegram(config) => self.send_telegram(config, message).await,
            AlertChannelConfig::Discord(config) => self.send_discord(config, message).await,
            AlertChannelConfig::Webhook(config) => self.send_webhook(config, message).await,
        }
    }

    async fn send_telegram(
        &self,
        config: &TelegramChannelConfig,
        message: &AlertMessage,
    ) -> Result<DeliveryReport, TianJiError> {
        let endpoint = telegram_endpoint(config);
        let redacted = redact_telegram_endpoint(&endpoint, &config.bot_token);
        let chunks = chunk_text(&message.text(), TELEGRAM_TEXT_LIMIT);
        for chunk in &chunks {
            let mut payload = json!({
                "chat_id": config.chat_id,
                "text": chunk,
            });
            if let Some(thread_id) = config.message_thread_id {
                payload["message_thread_id"] = json!(thread_id);
            }
            self.post_json(&endpoint, &redacted, &payload).await?;
        }
        Ok(DeliveryReport {
            channel_name: config.name.clone(),
            channel_kind: AlertChannelKind::Telegram,
            status: DeliveryStatus::Sent,
            chunks: chunks.len(),
            endpoint: redacted,
        })
    }

    async fn send_discord(
        &self,
        config: &DiscordChannelConfig,
        message: &AlertMessage,
    ) -> Result<DeliveryReport, TianJiError> {
        let redacted = redact_url(&config.webhook_url);
        let chunks = chunk_text(&message.text(), DISCORD_CONTENT_LIMIT);
        for chunk in &chunks {
            let payload = json!({ "content": chunk });
            self.post_json(&config.webhook_url, &redacted, &payload)
                .await?;
        }
        Ok(DeliveryReport {
            channel_name: config.name.clone(),
            channel_kind: AlertChannelKind::Discord,
            status: DeliveryStatus::Sent,
            chunks: chunks.len(),
            endpoint: redacted,
        })
    }

    async fn send_webhook(
        &self,
        config: &WebhookChannelConfig,
        message: &AlertMessage,
    ) -> Result<DeliveryReport, TianJiError> {
        let redacted = redact_url(&config.url);
        let payload = json!({
            "tier": message.tier,
            "title": message.title,
            "summary": message.summary,
            "body": message.body,
        });
        let mut request = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .body(serialize_payload(&payload)?);
        for (name, value) in &config.headers {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(|error| {
            TianJiError::Usage(format!(
                "alert dispatch failed for webhook channel '{}': request to {} failed: {}",
                config.name,
                redacted,
                redact_error(&error.to_string(), &config.url)
            ))
        })?;
        ensure_success(response, &config.name, AlertChannelKind::Webhook, &redacted).await?;
        Ok(DeliveryReport {
            channel_name: config.name.clone(),
            channel_kind: AlertChannelKind::Webhook,
            status: DeliveryStatus::Sent,
            chunks: 1,
            endpoint: redacted,
        })
    }

    async fn post_json(
        &self,
        endpoint: &str,
        redacted_endpoint: &str,
        payload: &serde_json::Value,
    ) -> Result<(), TianJiError> {
        let response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .body(serialize_payload(payload)?)
            .send()
            .await
            .map_err(|error| {
                TianJiError::Usage(format!(
                    "alert dispatch request to {} failed: {}",
                    redacted_endpoint,
                    redact_error(&error.to_string(), endpoint)
                ))
            })?;
        ensure_success(
            response,
            "unknown",
            AlertChannelKind::Webhook,
            redacted_endpoint,
        )
        .await
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchReport {
    pub dry_run: bool,
    pub deliveries: Vec<DeliveryReport>,
}

impl fmt::Display for DispatchReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "DispatchReport(dry_run={}, deliveries={:?})",
            self.dry_run, self.deliveries
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryReport {
    pub channel_name: String,
    pub channel_kind: AlertChannelKind,
    pub status: DeliveryStatus,
    pub chunks: usize,
    pub endpoint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryStatus {
    Planned,
    Sent,
}

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn serialize_payload(payload: &serde_json::Value) -> Result<String, TianJiError> {
    serde_json::to_string(payload).map_err(TianJiError::Json)
}

async fn ensure_success(
    response: reqwest::Response,
    channel_name: &str,
    channel_kind: AlertChannelKind,
    redacted_endpoint: &str,
) -> Result<(), TianJiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    Err(TianJiError::Usage(format!(
        "alert dispatch failed for {:?} channel '{}': HTTP {} from {}: {}",
        channel_kind,
        channel_name,
        status,
        redacted_endpoint,
        snippet(&body, HTTP_BODY_SNIPPET_LIMIT)
    )))
}

fn chunk_count(channel: &AlertChannelConfig, message: &AlertMessage) -> usize {
    match channel {
        AlertChannelConfig::Telegram(_) => chunk_text(&message.text(), TELEGRAM_TEXT_LIMIT).len(),
        AlertChannelConfig::Discord(_) => chunk_text(&message.text(), DISCORD_CONTENT_LIMIT).len(),
        AlertChannelConfig::Webhook(_) => 1,
    }
}

pub fn chunk_text(text: &str, char_limit: usize) -> Vec<String> {
    assert!(char_limit > 0, "chunk char limit must be non-zero");
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if current.chars().count() == char_limit {
            chunks.push(current);
            current = String::new();
        }
        current.push(character);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn telegram_endpoint(config: &TelegramChannelConfig) -> String {
    let base = config
        .api_base_url
        .as_deref()
        .unwrap_or("https://api.telegram.org")
        .trim_end_matches('/');
    format!("{}/bot{}/sendMessage", base, config.bot_token)
}

fn redacted_endpoint(channel: &AlertChannelConfig) -> String {
    match channel {
        AlertChannelConfig::Telegram(config) => {
            redact_telegram_endpoint(&telegram_endpoint(config), &config.bot_token)
        }
        AlertChannelConfig::Discord(config) => redact_url(&config.webhook_url),
        AlertChannelConfig::Webhook(config) => redact_url(&config.url),
    }
}

fn redact_telegram_endpoint(endpoint: &str, token: &str) -> String {
    endpoint.replace(token, "<redacted>")
}

fn redact_url(url: &str) -> String {
    match url.find("://") {
        Some(scheme_end) => format!("{}://<redacted>", &url[..scheme_end]),
        None => "<redacted>".to_string(),
    }
}

fn redacted_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .keys()
        .map(|name| (name.clone(), "<redacted>".to_string()))
        .collect()
}

fn redact_error(message: &str, secret: &str) -> String {
    message.replace(secret, "<redacted>")
}

fn snippet(text: &str, limit: usize) -> String {
    let mut result: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        result.push_str("...");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, Receiver};
    use std::thread;

    #[derive(Debug)]
    struct CapturedRequest {
        path: String,
        headers: String,
        body: serde_json::Value,
    }

    fn message(tier: AlertTier) -> AlertMessage {
        AlertMessage {
            tier,
            title: "Escalation".to_string(),
            summary: "Priority movement".to_string(),
            body: "Detailed signal body".to_string(),
        }
    }

    fn read_http_request(mut stream: &TcpStream) -> CapturedRequest {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let count = stream.read(&mut buffer).expect("read request bytes");
            assert!(count > 0, "client closed before headers");
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("headers terminator")
            + 4;
        let headers = String::from_utf8(bytes[..header_end].to_vec()).expect("headers utf8");
        let request_line = headers.lines().next().expect("request line");
        let path = request_line
            .split_whitespace()
            .nth(1)
            .expect("request path")
            .to_string();
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("content length number")
                })
            })
            .expect("content-length header");

        while bytes.len() - header_end < content_length {
            let count = stream.read(&mut buffer).expect("read request body");
            assert!(count > 0, "client closed before body");
            bytes.extend_from_slice(&buffer[..count]);
        }

        let body = &bytes[header_end..header_end + content_length];
        let body = serde_json::from_slice(body).expect("request body JSON");
        CapturedRequest {
            path,
            headers,
            body,
        }
    }

    fn spawn_mock_server(expected_requests: usize) -> (String, Receiver<Vec<CapturedRequest>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("mock server addr");
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().expect("accept request");
                requests.push(read_http_request(&stream));
                let body = r#"{"ok":true}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
            sender.send(requests).expect("send captured requests");
        });

        (format!("http://{addr}"), receiver)
    }

    fn dispatch_config(dry_run: bool, base_url: &str) -> AlertDispatchConfig {
        AlertDispatchConfig {
            dry_run,
            channels: vec![
                AlertChannelConfig::Telegram(TelegramChannelConfig {
                    name: "ops-telegram".to_string(),
                    bot_token: "123456:SECRET_TOKEN".to_string(),
                    chat_id: "chat-1".to_string(),
                    message_thread_id: Some(42),
                    api_base_url: Some(base_url.to_string()),
                }),
                AlertChannelConfig::Discord(DiscordChannelConfig {
                    name: "ops-discord".to_string(),
                    webhook_url: format!("{base_url}/discord/SECRET_WEBHOOK"),
                }),
                AlertChannelConfig::Webhook(WebhookChannelConfig {
                    name: "ops-webhook".to_string(),
                    url: format!("{base_url}/webhook/SECRET_WEBHOOK"),
                    headers: BTreeMap::from([(
                        "X-Static".to_string(),
                        "secret-header".to_string(),
                    )]),
                }),
            ],
            policy: AlertDispatchPolicy {
                flash: vec!["ops-telegram".to_string(), "ops-discord".to_string()],
                priority: vec!["ops-webhook".to_string()],
                routine: Vec::new(),
            },
        }
    }

    #[tokio::test]
    async fn alert_dispatch_dry_run_plans_deliveries_without_network() {
        let config = dispatch_config(true, "http://127.0.0.1:9");
        let dispatcher = AlertDispatcher::new(config);

        let report = dispatcher
            .dispatch(&message(AlertTier::Flash))
            .await
            .unwrap();

        assert!(report.dry_run);
        assert_eq!(report.deliveries.len(), 2);
        assert_eq!(report.deliveries[0].status, DeliveryStatus::Planned);
        assert_eq!(report.deliveries[0].channel_name, "ops-telegram");
        assert_eq!(report.deliveries[1].channel_name, "ops-discord");
    }

    #[tokio::test]
    async fn alert_dispatch_tier_policy_filters_channels() {
        let config = dispatch_config(true, "http://127.0.0.1:9");
        let dispatcher = AlertDispatcher::new(config);

        let priority = dispatcher
            .dispatch(&message(AlertTier::Priority))
            .await
            .unwrap();
        let routine = dispatcher
            .dispatch(&message(AlertTier::Routine))
            .await
            .unwrap();

        assert_eq!(priority.deliveries.len(), 1);
        assert_eq!(priority.deliveries[0].channel_name, "ops-webhook");
        assert!(routine.deliveries.is_empty());
    }

    #[test]
    fn alert_dispatch_telegram_and_discord_chunking_is_deterministic() {
        let telegram = chunk_text(&"a".repeat(TELEGRAM_TEXT_LIMIT + 1), TELEGRAM_TEXT_LIMIT);
        let discord = chunk_text(
            &"界".repeat(DISCORD_CONTENT_LIMIT + 1),
            DISCORD_CONTENT_LIMIT,
        );

        assert_eq!(telegram.len(), 2);
        assert_eq!(telegram[0].chars().count(), TELEGRAM_TEXT_LIMIT);
        assert_eq!(telegram[1], "a");
        assert_eq!(discord.len(), 2);
        assert_eq!(discord[0].chars().count(), DISCORD_CONTENT_LIMIT);
        assert_eq!(discord[1], "界");
    }

    #[tokio::test]
    async fn alert_dispatch_mock_http_captures_all_payload_shapes() {
        let (base_url, receiver) = spawn_mock_server(3);
        let config = AlertDispatchConfig {
            dry_run: false,
            channels: dispatch_config(false, &base_url).channels,
            policy: AlertDispatchPolicy {
                flash: vec![
                    "ops-telegram".to_string(),
                    "ops-discord".to_string(),
                    "ops-webhook".to_string(),
                ],
                priority: Vec::new(),
                routine: Vec::new(),
            },
        };
        let dispatcher = AlertDispatcher::new(config);

        let report = dispatcher
            .dispatch(&message(AlertTier::Flash))
            .await
            .unwrap();
        let requests = receiver.recv().expect("captured requests");

        assert_eq!(report.deliveries.len(), 3);
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].path, "/bot123456:SECRET_TOKEN/sendMessage");
        assert_eq!(requests[0].body["chat_id"], "chat-1");
        assert_eq!(requests[0].body["text"], message(AlertTier::Flash).text());
        assert_eq!(requests[0].body["message_thread_id"], 42);
        assert_eq!(requests[1].path, "/discord/SECRET_WEBHOOK");
        assert_eq!(
            requests[1].body["content"],
            message(AlertTier::Flash).text()
        );
        assert_eq!(requests[2].path, "/webhook/SECRET_WEBHOOK");
        assert!(requests[2]
            .headers
            .to_ascii_lowercase()
            .contains("x-static: secret-header"));
        assert_eq!(requests[2].body["tier"], "flash");
        assert_eq!(requests[2].body["title"], "Escalation");
        assert_eq!(requests[2].body["summary"], "Priority movement");
        assert_eq!(requests[2].body["body"], "Detailed signal body");
    }

    #[tokio::test]
    async fn alert_dispatch_contract_dry_run_report_redacts_endpoints() {
        let config = dispatch_config(true, "https://alerts.example.test/SECRET_BASE");
        let dispatcher = AlertDispatcher::new(config);

        let report = dispatcher
            .dispatch(&message(AlertTier::Flash))
            .await
            .expect("dry-run dispatch report");

        assert!(report.dry_run);
        assert_eq!(report.deliveries.len(), 2);
        assert_eq!(report.deliveries[0].channel_name, "ops-telegram");
        assert_eq!(
            report.deliveries[0].channel_kind,
            AlertChannelKind::Telegram
        );
        assert_eq!(report.deliveries[0].status, DeliveryStatus::Planned);
        assert_eq!(report.deliveries[0].chunks, 1);
        assert_eq!(
            report.deliveries[0].endpoint,
            "https://alerts.example.test/SECRET_BASE/bot<redacted>/sendMessage"
        );
        assert_eq!(report.deliveries[1].channel_name, "ops-discord");
        assert_eq!(report.deliveries[1].channel_kind, AlertChannelKind::Discord);
        assert_eq!(report.deliveries[1].status, DeliveryStatus::Planned);
        assert_eq!(report.deliveries[1].endpoint, "https://<redacted>");

        let rendered = format!("{report}");
        assert!(!rendered.contains("123456:SECRET_TOKEN"));
        assert!(!rendered.contains("SECRET_WEBHOOK"));
        assert!(rendered.contains("<redacted>"));
    }

    #[tokio::test]
    async fn alert_dispatch_contract_mock_webhook_payload_shape() {
        let (base_url, receiver) = spawn_mock_server(1);
        let config = AlertDispatchConfig {
            dry_run: false,
            channels: dispatch_config(false, &base_url).channels,
            policy: AlertDispatchPolicy {
                flash: Vec::new(),
                priority: vec!["ops-webhook".to_string()],
                routine: Vec::new(),
            },
        };
        let dispatcher = AlertDispatcher::new(config);

        let report = dispatcher
            .dispatch(&message(AlertTier::Priority))
            .await
            .expect("mock webhook dispatch");
        let requests = receiver.recv().expect("captured request");

        assert_eq!(report.deliveries.len(), 1);
        assert_eq!(report.deliveries[0].channel_kind, AlertChannelKind::Webhook);
        assert_eq!(report.deliveries[0].status, DeliveryStatus::Sent);
        assert_eq!(report.deliveries[0].endpoint, "http://<redacted>");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/webhook/SECRET_WEBHOOK");
        assert!(requests[0]
            .headers
            .to_ascii_lowercase()
            .contains("content-type: application/json"));
        assert_eq!(
            requests[0].body,
            serde_json::json!({
                "tier": "priority",
                "title": "Escalation",
                "summary": "Priority movement",
                "body": "Detailed signal body"
            })
        );
    }

    #[tokio::test]
    async fn alert_dispatch_redacts_secrets_in_debug_report_and_errors() {
        let secret_token = "123456:SECRET_TOKEN";
        let secret_webhook = "SECRET_WEBHOOK";
        let config = dispatch_config(true, "http://127.0.0.1:9");
        let debug = format!("{config:?}");
        assert!(!debug.contains(secret_token));
        assert!(!debug.contains(secret_webhook));
        assert!(debug.contains("<redacted>"));

        let dispatcher = AlertDispatcher::new(dispatch_config(false, "http://127.0.0.1:9"));
        let error = dispatcher
            .dispatch(&message(AlertTier::Flash))
            .await
            .expect_err("closed local port should fail")
            .to_string();
        assert!(!error.contains(secret_token));
        assert!(!error.contains(secret_webhook));
        assert!(error.contains("<redacted>"));
    }
}
