//! Enrichment via a local **OpenAI-compatible** chat server (LM Studio, Ollama,
//! or llama.cpp's OpenAI mode). Compiled only under the `enrich` feature so the
//! default build pulls in no HTTP stack.
//!
//! The model is a scoped spoke in the hub-and-spoke pattern: it does one bounded
//! job — read a markdown note, return a small JSON object — and the deterministic
//! core stays the authority. Output validity is constrained by an OpenAI
//! `response_format` json_schema (mirroring [`model::Enrichment`]), not by hoping
//! the model formats JSON correctly. Temperature is 0 so the same note yields the
//! same extraction.
//!
//! Endpoint and model id are configurable:
//!   - `PHANES_LLM_URL`   (default `http://127.0.0.1:1234/v1/chat/completions`)
//!   - `PHANES_LLM_MODEL` (default `local-model`; LM Studio serves whatever model
//!      is loaded, so the id is often ignored — pin it for other servers)
//!
//! See `D-012` for why this targets the OpenAI-compatible API rather than
//! llama.cpp's native `/completion` + GBNF (`grammars/idea_extract.gbnf` remains
//! for that alternative path).

use anyhow::{Context, Result};
use serde_json::json;

use crate::model::Enrichment;

const DEFAULT_URL: &str = "http://127.0.0.1:1234/v1/chat/completions";
const DEFAULT_MODEL: &str = "local-model";

const SYSTEM_PROMPT: &str = "You catalogue project-idea notes. Read the note and \
    return JSON only, matching the schema. summary: one plain sentence describing \
    what the note is about. status: one of the allowed values. tags: 2-6 short \
    lowercase keywords. topics: 1-4 broader concept areas.";

fn endpoint() -> String {
    std::env::var("PHANES_LLM_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn model_id() -> String {
    std::env::var("PHANES_LLM_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

/// The `response_format` json_schema that constrains the reply to exactly
/// [`model::Enrichment`]. Keep the `status` enum in lockstep with
/// [`crate::model::Status`].
fn response_format() -> serde_json::Value {
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "enrichment",
            "strict": true,
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["summary", "status", "tags", "topics"],
                "properties": {
                    "summary": { "type": "string" },
                    "status": {
                        "type": "string",
                        "enum": [
                            "concept", "draft", "active", "dormant",
                            "complete", "archived", "superseded", "unknown"
                        ]
                    },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "topics": { "type": "array", "items": { "type": "string" } }
                }
            }
        }
    })
}

/// The note as the user turn. Long bodies are truncated on a char boundary — the
/// first ~6k chars carry the gist of an idea note.
fn user_message(title: &str, body: &str) -> String {
    let body = if body.len() > 6000 {
        let mut end = 6000;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        &body[..end]
    } else {
        body
    };
    format!("# {title}\n{body}")
}

/// One chat-completion round trip; returns the assistant message content.
/// Shared by extraction (with a json_schema) and bridge proposal (freeform).
fn chat(system: &str, user: &str, response_format: Option<serde_json::Value>, max_tokens: u32) -> Result<String> {
    let mut payload = json!({
        "model": model_id(),
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": 0.0,
        "max_tokens": max_tokens
    });
    if let Some(rf) = response_format {
        payload["response_format"] = rf;
    }

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(endpoint())
        .json(&payload)
        .send()
        .context("POST to the local model server failed (is LM Studio / the server running?)")?
        .error_for_status()?;

    let raw: serde_json::Value = resp.json().context("model server returned non-JSON")?;
    raw.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .context("no choices[0].message.content in the model server response")
}

/// Run extraction against the local model. Returns `Err` on any transport or
/// parse failure so the caller can fall back to an asserted-only record (INV-4).
pub fn enrich(title: &str, body: &str) -> Result<Enrichment> {
    let content = chat(SYSTEM_PROMPT, &user_message(title, body), Some(response_format()), 400)?;
    parse_enrichment(&content)
}

const BRIDGE_SYSTEM: &str = "You connect project ideas. Given two notes, propose \
    ONE concrete new idea or project that bridges them — something that genuinely \
    draws on both. Answer in 1-3 plain sentences, no preamble, no restating the \
    inputs.";

/// Propose a bridging idea connecting two notes. **On-demand / user-invoked**
/// (e.g. the `bridge` command) — not part of the instant query paths, see D-015.
/// `Err` on any failure so the caller can report it without crashing.
pub fn propose_bridge(a_title: &str, a_body: &str, b_title: &str, b_body: &str) -> Result<String> {
    let user = format!(
        "Note A — {a_title}\n{}\n\nNote B — {b_title}\n{}\n\nPropose one idea that bridges A and B.",
        truncate(a_body, 2500),
        truncate(b_body, 2500),
    );
    Ok(chat(BRIDGE_SYSTEM, &user, None, 220)?.trim().to_string())
}

/// Truncate to at most `max` bytes on a char boundary.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Parse the model's JSON reply into an [`Enrichment`]. Split out so it can be
/// tested without a live server.
fn parse_enrichment(content: &str) -> Result<Enrichment> {
    serde_json::from_str::<Enrichment>(content.trim())
        .context("model output did not match the Enrichment schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    #[test]
    fn parses_a_well_formed_reply() {
        let content = r#"{"summary":"A spatial canvas for ideas.","status":"active","tags":["ui","spatial"],"topics":["visualization"]}"#;
        let e = parse_enrichment(content).unwrap();
        assert_eq!(e.summary, "A spatial canvas for ideas.");
        assert_eq!(e.status, Status::Active);
        assert_eq!(e.tags, vec!["ui", "spatial"]);
        assert_eq!(e.topics, vec!["visualization"]);
    }

    #[test]
    fn parses_the_new_concept_status() {
        let content = r#"{"summary":"x","status":"concept","tags":[],"topics":[]}"#;
        assert_eq!(parse_enrichment(content).unwrap().status, Status::Concept);
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        let content = "\n  {\"summary\":\"x\",\"status\":\"draft\",\"tags\":[],\"topics\":[]}  \n";
        assert!(parse_enrichment(content).is_ok());
    }

    #[test]
    fn rejects_malformed_reply() {
        assert!(parse_enrichment("not json at all").is_err());
    }

    #[test]
    fn user_message_truncates_on_char_boundary() {
        let body = "é".repeat(5000); // 10k bytes, multibyte
        let msg = user_message("T", &body);
        assert!(msg.is_char_boundary(msg.len())); // didn't panic / split a char
    }
}
