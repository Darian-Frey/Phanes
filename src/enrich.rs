//! Enrichment via a local `llama-server` (llama.cpp). Compiled only under the
//! `enrich` feature so the default build pulls in no HTTP stack.
//!
//! The model is treated as a scoped spoke in the hub-and-spoke pattern: it does
//! one bounded job — read a markdown file, return a small JSON object — and the
//! deterministic core stays the authority. Output validity is guaranteed by a
//! GBNF grammar (constrained decoding), not by hoping the model formats JSON
//! correctly. Temperature is 0 so the same file yields the same extraction.

use anyhow::{Context, Result};
use serde_json::json;

use crate::model::Enrichment;

/// llama-server default endpoint. Override with PHANES_LLAMA_URL.
const DEFAULT_URL: &str = "http://127.0.0.1:8080/completion";

/// The grammar that constrains output to exactly `model::Enrichment`.
const GRAMMAR: &str = include_str!("../grammars/idea_extract.gbnf");

fn endpoint() -> String {
    std::env::var("PHANES_LLAMA_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn build_prompt(title: &str, body: &str) -> String {
    // Keep it small; the task is light. Truncate very long bodies — the first
    // ~6k chars carry the gist of an idea note.
    let body = if body.len() > 6000 { &body[..6000] } else { body };
    format!(
        "You catalogue project-idea notes. Read the note and return JSON only.\n\
         - summary: one sentence, plain.\n\
         - status: one of active, dormant, complete, archived, superseded, unknown.\n\
         - tags: 2-6 short lowercase keywords.\n\
         - topics: 1-4 broader concept areas.\n\n\
         # {title}\n{body}\n"
    )
}

/// Run extraction. Returns `Err` on any transport or parse failure so the
/// caller can fall back to an asserted-only record.
pub fn enrich(title: &str, body: &str) -> Result<Enrichment> {
    let payload = json!({
        "prompt": build_prompt(title, body),
        "grammar": GRAMMAR,
        "temperature": 0.0,
        "n_predict": 400,
        "cache_prompt": true
    });

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(endpoint())
        .json(&payload)
        .send()
        .context("POST to llama-server failed (is it running?)")?
        .error_for_status()?;

    // llama.cpp /completion returns { "content": "<the generated text>" , ... }
    let raw: serde_json::Value = resp.json().context("llama-server returned non-JSON")?;
    let content = raw
        .get("content")
        .and_then(|c| c.as_str())
        .context("no `content` field in llama-server response")?;

    serde_json::from_str::<Enrichment>(content.trim())
        .context("model output did not match the Enrichment schema")
}
