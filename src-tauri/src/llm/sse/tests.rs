// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! Tests for SSE accumulators, the parser and the collector.
use super::*;
use crate::llm::provider::LLMError;
use serde_json::Value;

use serde_json::json;

#[test]
fn timeout_read_error_is_actionable_and_mentions_bound() {
    let msg = describe_sse_read_error(true, "error decoding response body");
    // Names the timeout cause and the configured bound, not the opaque
    // reqwest Display alone.
    assert!(msg.contains("timed out"));
    assert!(msg.contains(&crate::constants::llm_http::DEFAULT_READ_TIMEOUT_SECS.to_string()));
    assert!(msg.contains("error decoding response body"));
}

#[test]
fn non_timeout_read_error_keeps_generic_message() {
    let msg = describe_sse_read_error(false, "connection reset by peer");
    assert!(msg.starts_with("SSE stream read failed:"));
    assert!(msg.contains("connection reset by peer"));
    assert!(!msg.contains("timed out"));
}

fn ingest_all<A: DeltaAccumulator>(mut acc: A, chunks: Vec<Value>) -> Value {
    for c in &chunks {
        acc.ingest(c);
    }
    Box::new(acc).finalize()
}

// ---------------- OpenAiAccumulator ----------------

#[test]
fn test_openai_accumulate_simple_content() {
    let chunks = vec![
        json!({"id":"x","model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":"He"},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":"llo"},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":" world"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(out["choices"][0]["finish_reason"], "stop");
    assert_eq!(out["id"], "x");
    assert_eq!(out["model"], "m");
}

#[test]
fn test_openai_accumulate_reasoning_string_separate() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning":"Let me "},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning":"think..."},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":"Answer"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["message"]["reasoning"], "Let me think...");
    assert_eq!(out["choices"][0]["message"]["content"], "Answer");
}

#[test]
fn test_openai_accumulate_reasoning_content_alias() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning_content":"part1 "}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_content":"part2"}}]}),
        json!({"choices":[{"index":0,"delta":{"content":"X"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    // The reasoning must be echoed back under the SAME field it arrived in
    // (`reasoning_content`), not renamed to `reasoning`: DeepSeek (incl. via
    // RouterLab) returns HTTP 400 on a tool-loop follow-up turn whose
    // reasoning is renamed. Regression guard for that round-trip.
    assert_eq!(
        out["choices"][0]["message"]["reasoning_content"],
        "part1 part2"
    );
    assert!(out["choices"][0]["message"].get("reasoning").is_none());
}

#[test]
fn test_openai_reasoning_field_name_preserved_for_round_trip() {
    // `reasoning` (vLLM) stays `reasoning`; `reasoning_content` (DeepSeek)
    // stays `reasoning_content`. First key seen wins when both appear.
    let r = ingest_all(
        OpenAiAccumulator::new(),
        vec![json!({"choices":[{"index":0,"delta":{"reasoning":"a"},"finish_reason":"stop"}]})],
    );
    assert_eq!(r["choices"][0]["message"]["reasoning"], "a");
    assert!(r["choices"][0]["message"]
        .get("reasoning_content")
        .is_none());

    let rc = ingest_all(
        OpenAiAccumulator::new(),
        vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_content":"b"},"finish_reason":"stop"}]}),
        ],
    );
    assert_eq!(rc["choices"][0]["message"]["reasoning_content"], "b");
    assert!(rc["choices"][0]["message"].get("reasoning").is_none());
}

#[test]
fn test_openai_accumulate_reasoning_details_array() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"step 1...","index":0}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"step 2...","index":1}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"step 3...","index":2}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"content":"final"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    let details = out["choices"][0]["message"]["reasoning_details"]
        .as_array()
        .unwrap();
    assert_eq!(details.len(), 3);
    assert_eq!(details[0]["text"], "step 1...");
    assert_eq!(details[2]["text"], "step 3...");
}

#[test]
fn test_openai_accumulate_reasoning_redacted_passthrough() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.encrypted","text":"[REDACTED]","signature":"abc"}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    let details = out["choices"][0]["message"]["reasoning_details"]
        .as_array()
        .unwrap();
    assert_eq!(details[0]["text"], "[REDACTED]");
    assert_eq!(details[0]["signature"], "abc");
}

/// Custom-provider regression: a few OpenAI-compat front-ends (Ollama
/// behind a reverse-proxy, certain forks) publish the reasoning trace at
/// `delta.thinking` instead of `delta.reasoning`. Without this bucket,
/// the Reflexion UI block never appears for those custom providers.
#[test]
fn test_openai_accumulate_thinking_field_string() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"thinking":"step 1 "}}]}),
        json!({"choices":[{"index":0,"delta":{"thinking":"step 2"}}]}),
        json!({"choices":[{"index":0,"delta":{"content":"Answer."},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["message"]["thinking"], "step 1 step 2");
    let message = out.pointer("/choices/0/message").unwrap();
    let thinking = crate::llm::utils::extract_thinking_from_message(message);
    assert_eq!(thinking.as_deref(), Some("step 1 step 2"));
}

#[test]
fn test_openai_accumulate_reasoning_str_and_details_coexist() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning":"raw "}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[{"type":"reasoning.text","text":"typed"}]}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning":"more"}}]}),
        json!({"choices":[{"index":0,"delta":{"content":"a"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["message"]["reasoning"], "raw more");
    assert_eq!(
        out["choices"][0]["message"]["reasoning_details"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn test_openai_accumulate_tool_calls_single() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"id":"call_1","type":"function","function":{"name":"my_tool","arguments":""}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"function":{"arguments":"{\"a\":"}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"function":{"arguments":"1}"}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    let calls = out["choices"][0]["message"]["tool_calls"]
        .as_array()
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["id"], "call_1");
    assert_eq!(calls[0]["type"], "function");
    assert_eq!(calls[0]["function"]["name"], "my_tool");
    assert_eq!(calls[0]["function"]["arguments"], "{\"a\":1}");
    // Arguments must be JSON-parsable.
    let _: Value =
        serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
}

#[test]
fn test_openai_accumulate_tool_calls_interleaved() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"id":"a","type":"function","function":{"name":"first","arguments":"{\""}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":1,"id":"b","type":"function","function":{"name":"second","arguments":"{\""}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"function":{"arguments":"x\":1}"}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":1,"function":{"arguments":"y\":2}"}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    let calls = out["choices"][0]["message"]["tool_calls"]
        .as_array()
        .unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["id"], "a");
    assert_eq!(calls[0]["function"]["name"], "first");
    assert_eq!(calls[0]["function"]["arguments"], "{\"x\":1}");
    assert_eq!(calls[1]["id"], "b");
    assert_eq!(calls[1]["function"]["name"], "second");
    assert_eq!(calls[1]["function"]["arguments"], "{\"y\":2}");
}

#[test]
fn test_openai_finalize_omits_empty_tool_calls() {
    let chunks =
        vec![json!({"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":"stop"}]})];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert!(out["choices"][0]["message"].get("tool_calls").is_none());
}

#[test]
fn test_openai_finalize_includes_usage_when_present() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":"stop"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert_eq!(out["usage"]["prompt_tokens"], 12);
    assert_eq!(out["usage"]["completion_tokens"], 5);
}

/// End-to-end check that the OpenRouter prompt caching breakpoints
/// applied via `cache_control.rs` survive the streaming round-trip.
/// The provider replies with the detailed `usage.prompt_tokens_details.*`
/// shape (cached_tokens + cache_write_tokens) and the accumulator must
/// preserve every sub-pointer so `tool_adapter::extract_usage` reads
/// the same values it would have read non-streaming. Without this the
/// cost calculation would silently fall back to "no cache hits".
#[test]
fn test_openai_finalize_preserves_cache_control_usage_details() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":"stop"}]}),
        // Final usage chunk modeled on a real OpenRouter response for
        // Anthropic Claude with prompt caching active.
        json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 5000,
                "completion_tokens": 800,
                "total_tokens": 5800,
                "prompt_tokens_details": {
                    "cached_tokens": 4200,
                    "cache_write_tokens": 600,
                    "audio_tokens": 0
                },
                "completion_tokens_details": {
                    "reasoning_tokens": 250
                },
                "cost": 0.012345
            }
        }),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    // Every pointer read by `tool_adapter::extract_usage` must resolve.
    assert_eq!(out.pointer("/usage/prompt_tokens").unwrap(), 5000);
    assert_eq!(out.pointer("/usage/completion_tokens").unwrap(), 800);
    assert_eq!(
        out.pointer("/usage/prompt_tokens_details/cached_tokens")
            .unwrap(),
        4200
    );
    assert_eq!(
        out.pointer("/usage/prompt_tokens_details/cache_write_tokens")
            .unwrap(),
        600
    );
    assert_eq!(
        out.pointer("/usage/completion_tokens_details/reasoning_tokens")
            .unwrap(),
        250
    );
    assert!((out.pointer("/usage/cost").unwrap().as_f64().unwrap() - 0.012345).abs() < 1e-9);
}

/// Regression: when the assistant turn carries `reasoning_details` (the
/// Anthropic-via-OpenRouter case where each thinking block has a
/// cryptographic `signature`), the accumulator must surface it on
/// `message.reasoning_details` so the next iteration can echo the
/// blocks back unchanged. Without this the prompt-cache breakpoint
/// applied by `cache_control.rs` would land on an assistant message
/// missing its signed reasoning, and OpenRouter would reject the
/// follow-up request.
#[test]
fn test_openai_finalize_reasoning_details_round_trip_for_cache_echo() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"Step A","signature":"sig1"}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"Step B","signature":"sig2"}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"content":"Answer"},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    let details = out
        .pointer("/choices/0/message/reasoning_details")
        .and_then(|v| v.as_array())
        .expect("reasoning_details must be present at the message level");
    assert_eq!(details.len(), 2);
    // Both signatures preserved verbatim — OpenRouter validates these
    // when the assistant message is replayed in the next turn.
    assert_eq!(details[0]["signature"], "sig1");
    assert_eq!(details[1]["signature"], "sig2");
}

#[test]
fn test_openai_finalize_omits_usage_when_absent() {
    let chunks =
        vec![json!({"choices":[{"index":0,"delta":{"content":"x"},"finish_reason":"stop"}]})];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert!(out.get("usage").is_none());
}

#[test]
fn test_openai_finish_reason_takes_last_non_null() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":"a"},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":"b"},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];
    let out = ingest_all(OpenAiAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
}

// ---------------- MistralAccumulator ----------------

#[test]
fn test_mistral_accumulate_text_chunk_concat() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"role":"assistant","content":{"type":"text","text":"Hel"}},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"lo"}},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":" world"}},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let content = out["choices"][0]["message"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Hello world");
}

#[test]
fn test_mistral_accumulate_thinking_chunk_open_then_closed() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":false,
            "thinking":[{"type":"text","text":"ref"}]
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":false,
            "thinking":[{"type":"text","text":"lexion..."}]
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":true,
                "thinking":[]
            }},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let content = out["choices"][0]["message"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "thinking");
    let inner = content[0]["thinking"].as_array().unwrap();
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0]["text"], "reflexion...");
}

#[test]
fn test_mistral_accumulate_thinking_then_text() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":true,
            "thinking":[{"type":"text","text":"plan"}]
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{
                "type":"text","text":"answer"
            }},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let content = out["choices"][0]["message"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "thinking");
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "answer");
}

#[test]
fn test_mistral_tool_calls_are_openai_compat() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"id":"call_1","type":"function","function":{"name":"f","arguments":"{"}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"function":{"arguments":"\"k\":1}"}}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let calls = out["choices"][0]["message"]["tool_calls"]
        .as_array()
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["id"], "call_1");
    assert_eq!(calls[0]["function"]["name"], "f");
    assert_eq!(calls[0]["function"]["arguments"], "{\"k\":1}");
}

/// Regression: mistral-small-3.5 / mistral-medium-3.5 ship the
/// `ThinkChunk.thinking` payload as a **plain string** instead of the
/// Magistral array form. Without explicit handling, the original
/// accumulator silently dropped the entire thinking text — the
/// Reflexion UI block never appeared in the frontend.
#[test]
fn test_mistral_accumulate_thinking_string_form() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":false,
            "thinking":"L'utilisateur "
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":false,
            "thinking":"demande X."
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":true,
            "thinking":""
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{
                "type":"text","text":"Voici la reponse."
            }},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let content = out["choices"][0]["message"]["content"].as_array().unwrap();
    assert_eq!(
        content.len(),
        2,
        "expected exactly one Thinking block + one Text block, got: {:?}",
        content
    );
    assert_eq!(content[0]["type"], "thinking");
    // Normalized into the array form so extract_thinking_from_message can
    // walk the content array uniformly.
    let inner = content[0]["thinking"].as_array().unwrap();
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0]["text"], "L'utilisateur demande X.");
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "Voici la reponse.");
}

/// Regression: some Mistral routes (notably when the model is relayed
/// through OpenRouter or a vLLM-shaped fork) publish reasoning at
/// `delta.reasoning` instead of `delta.content` ThinkChunks. The
/// accumulator must mirror it back as `message.reasoning` so the
/// shared extractor surfaces it.
#[test]
fn test_mistral_accumulate_top_level_reasoning_field() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning":"Step 1. "}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning":"Step 2."}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"answer"}},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["message"]["reasoning"], "Step 1. Step 2.");
}

/// Same as above but for `reasoning_content` (LM Studio / vLLM alias).
#[test]
fn test_mistral_accumulate_top_level_reasoning_content_alias() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning_content":"part1 "}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_content":"part2"}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"x"}},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    assert_eq!(out["choices"][0]["message"]["reasoning"], "part1 part2");
}

/// Same as above but for the `delta.thinking` string variant
/// (Ollama-relayed Mistral and some proxies).
#[test]
fn test_mistral_accumulate_top_level_thinking_field() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"thinking":"reflexion "}}]}),
        json!({"choices":[{"index":0,"delta":{"thinking":"interne"}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"final"}},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    assert_eq!(
        out["choices"][0]["message"]["thinking"],
        "reflexion interne"
    );
}

/// `delta.reasoning_details[]` array form (OpenRouter-relayed Mistral).
#[test]
fn test_mistral_accumulate_top_level_reasoning_details_array() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"step A"}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"reasoning_details":[
            {"type":"reasoning.text","text":"step B"}
        ]}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"x"}},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let details = out["choices"][0]["message"]["reasoning_details"]
        .as_array()
        .unwrap();
    assert_eq!(details.len(), 2);
    assert_eq!(details[0]["text"], "step A");
    assert_eq!(details[1]["text"], "step B");
}

/// End-to-end check that the finalized JSON is consumed correctly by
/// the shared `extract_thinking_from_message` for the new mistral-small/
/// medium 3.5 string-form thinking. This is the test that would have
/// caught the regression from the user's report.
#[test]
fn test_mistral_string_form_thinking_is_extracted_by_helper() {
    let chunks = vec![
        json!({"choices":[{"index":0,"delta":{"content":{
            "type":"thinking","closed":true,
            "thinking":"Internal trace."
        }}}]}),
        json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"OK."}},"finish_reason":"stop"}]}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    let message = out.pointer("/choices/0/message").unwrap();
    let thinking = crate::llm::utils::extract_thinking_from_message(message);
    assert_eq!(thinking.as_deref(), Some("Internal trace."));
}

#[test]
fn test_mistral_finalize_matches_non_stream_shape() {
    // Sample inspired by Mistral's non-stream `reasoning_effort: high` body.
    let chunks = vec![
        json!({"id":"chat-1","model":"magistral-medium","created":1234,"choices":[{"index":0,"delta":{
                "role":"assistant",
                "content":{"type":"thinking","closed":true,"thinking":[{"type":"text","text":"r1"}]}
            },"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{
                "content":{"type":"text","text":"final"}
            },"finish_reason":"stop"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":3}}),
    ];
    let out = ingest_all(MistralAccumulator::new(), chunks);
    // Same pointers as the non-stream body. Adapters look at:
    // /choices/0/message/content (array), /usage/prompt_tokens, etc.
    assert_eq!(out["id"], "chat-1");
    assert_eq!(out["model"], "magistral-medium");
    assert_eq!(out["choices"][0]["message"]["role"], "assistant");
    let content = out["choices"][0]["message"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "thinking");
    assert_eq!(content[1]["type"], "text");
    assert_eq!(out["usage"]["prompt_tokens"], 7);
    assert_eq!(out["usage"]["completion_tokens"], 3);
    assert_eq!(out["choices"][0]["finish_reason"], "stop");
}

// ---------------- Parser SSE ----------------

#[test]
fn test_parse_data_line_simple() {
    let mut p = SseParser::new();
    let r = p.feed(b"data: {\"a\":1}\n\n").expect("feed ok");
    assert_eq!(r.events, vec!["{\"a\":1}".to_string()]);
    assert!(!r.done);
}

#[test]
fn test_parse_handles_chunked_event() {
    let mut p = SseParser::new();
    let r1 = p.feed(b"data: {\"a\"").expect("feed ok");
    assert!(r1.events.is_empty());
    let r2 = p.feed(b":1}\n\n").expect("feed ok");
    assert_eq!(r2.events, vec!["{\"a\":1}".to_string()]);
}

#[test]
fn test_parse_done_terminator() {
    let mut p = SseParser::new();
    let r = p
        .feed(b"data: {\"a\":1}\n\ndata: [DONE]\n\n")
        .expect("feed ok");
    assert_eq!(r.events, vec!["{\"a\":1}".to_string()]);
    assert!(r.done);
}

#[test]
fn test_parse_ignores_comments_and_keepalive() {
    let mut p = SseParser::new();
    let r = p
        .feed(b": keepalive\n\ndata: {\"x\":2}\n\n")
        .expect("feed ok");
    assert_eq!(r.events, vec!["{\"x\":2}".to_string()]);
}

#[test]
fn test_parse_handles_multi_event_in_one_chunk() {
    let mut p = SseParser::new();
    let r = p
        .feed(b"data: {\"a\":1}\n\ndata: {\"b\":2}\n\n")
        .expect("feed ok");
    assert_eq!(
        r.events,
        vec!["{\"a\":1}".to_string(), "{\"b\":2}".to_string()]
    );
}

#[test]
fn test_parse_handles_crlf() {
    let mut p = SseParser::new();
    let r = p.feed(b"data: {\"a\":1}\r\n\r\n").expect("feed ok");
    assert_eq!(r.events, vec!["{\"a\":1}".to_string()]);
}

#[test]
fn test_parse_strips_optional_space_after_colon() {
    let mut p = SseParser::new();
    // Per spec, "data: x" and "data:x" are both valid.
    let r1 = p.feed(b"data:no_space\n\n").expect("feed ok");
    assert_eq!(r1.events, vec!["no_space".to_string()]);
    let r2 = p.feed(b"data: with_space\n\n").expect("feed ok");
    assert_eq!(r2.events, vec!["with_space".to_string()]);
}

#[test]
fn test_parse_buffer_overflow_rejected() {
    // A misbehaving upstream that never sends \n\n must not cause unbounded
    // memory growth. Feeding > MAX_SSE_BUFFER_BYTES of data without an
    // event terminator yields LLMError::ResponseTooLarge.
    let mut p = SseParser::new();
    let big = vec![b'a'; MAX_SSE_BUFFER_BYTES + 1];
    let result = p.feed(&big);
    assert!(matches!(
        result,
        Err(LLMError::ResponseTooLarge { what: "SSE buffer" })
    ));
}

#[tokio::test]
async fn test_collect_sse_payload_overflow_rejected() {
    // A single SSE event whose payload exceeds MAX_SSE_PAYLOAD_BYTES is
    // rejected before serde_json::from_str is invoked.
    let payload: String = "x".repeat(MAX_SSE_PAYLOAD_BYTES + 1);
    // Build a static body so fake_sse_response can use it.
    let body: &'static str = Box::leak(format!("data: {}\n\n", payload).into_boxed_str());
    let resp = fake_sse_response(body);
    let result = collect_sse_to_json(resp, ProviderWireFormat::OpenAi).await;
    assert!(matches!(
        result,
        Err(LLMError::ResponseTooLarge {
            what: "single payload"
        })
    ));
}

// ---------------- collect_sse_to_json (integration) ----------------

/// Build a `reqwest::Response` from an in-memory SSE body without doing
/// any network I/O. Lets the integration tests below exercise the full
/// `bytes_stream` + parser + accumulator pipeline.
fn fake_sse_response(body: &'static str) -> reqwest::Response {
    let http_resp = http::Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(body)
        .expect("test http::Response");
    reqwest::Response::from(http_resp)
}

#[tokio::test]
async fn test_collect_sse_to_json_openai_full_round_trip() {
    let body = concat!(
            "data: {\"id\":\"x\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"He\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"llo\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2}}\n\n",
            "data: [DONE]\n\n",
        );
    let resp = fake_sse_response(body);
    let out = collect_sse_to_json(resp, ProviderWireFormat::OpenAi)
        .await
        .unwrap();
    assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(out["choices"][0]["finish_reason"], "stop");
    assert_eq!(out["usage"]["prompt_tokens"], 4);
    assert_eq!(out["usage"]["completion_tokens"], 2);
}

#[tokio::test]
async fn test_collect_sse_to_json_mistral_thinking_then_text() {
    let body = concat!(
            "data: {\"id\":\"x\",\"model\":\"magistral\",\"choices\":[{\"index\":0,\"delta\":{",
            "\"role\":\"assistant\",\"content\":{\"type\":\"thinking\",\"closed\":true,",
            "\"thinking\":[{\"type\":\"text\",\"text\":\"plan\"}]}},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":{\"type\":\"text\",\"text\":\"ok\"}},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
    let resp = fake_sse_response(body);
    let out = collect_sse_to_json(resp, ProviderWireFormat::Mistral)
        .await
        .unwrap();
    let content = out["choices"][0]["message"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "thinking");
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "ok");
}

#[tokio::test]
async fn test_collect_sse_to_json_rejects_malformed_chunk() {
    let body = "data: {\"oops\": notjson}\n\n";
    let resp = fake_sse_response(body);
    let err = collect_sse_to_json(resp, ProviderWireFormat::OpenAi)
        .await
        .unwrap_err();
    // Surface a structured `RequestFailed` error rather than panicking
    // or returning a half-baked accumulator.
    assert!(matches!(err, LLMError::RequestFailed(_)));
}
