//! The on-demand carrier as an MCP server on stdio: the genome as a
//! resource and a prompt, expression as a tool, each answered on the
//! on-demand carrier. Messages are JSON-RPC 2.0, one per line.

use crate::entry::{self, Call, Carrier, Ctx, Item, Slice};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub const GENOME_URI: &str = "my-harness://genome";
/// The protocol versions this server speaks, newest first. It uses only
/// resources, prompts and tools with text content, which each of them has;
/// a client asking for another is answered with the newest.
const VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// What the server knows of its session: the id, when the shell names it,
/// and the folder it runs in.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub id: Option<String>,
    pub cwd: Option<PathBuf>,
}

/// Answer every request on `input` until it ends.
pub fn serve(
    ctx: &Ctx,
    session: &Session,
    input: impl BufRead,
    mut out: impl Write,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Err(e) => Some(error(Value::Null, -32700, &format!("not JSON: {e}"))),
            Ok(msg) => answer(ctx, session, &msg),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// The reply to one message; none for a notification.
pub fn answer(ctx: &Ctx, session: &Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let params = &msg["params"];
    let result = match msg["method"].as_str().unwrap_or_default() {
        "initialize" => Ok(json!({
            "protocolVersion": params["protocolVersion"]
                .as_str()
                .filter(|v| VERSIONS.contains(v))
                .unwrap_or(VERSIONS[0]),
            "capabilities": {"resources": {}, "prompts": {}, "tools": {}},
            "serverInfo": {"name": "my-harness", "version": env!("CARGO_PKG_VERSION")},
        })),
        "ping" => Ok(json!({})),
        "resources/list" => Ok(json!({"resources": [{
            "uri": GENOME_URI, "name": "genome", "mimeType": "text/plain",
            "description": "Who this session works for and what it carries: personas, bring-ins, environment, pending.",
        }]})),
        "resources/read" => match params["uri"].as_str() {
            Some(GENOME_URI) => genome(ctx, session)
                .map(|text| json!({"contents": [{"uri": GENOME_URI, "mimeType": "text/plain", "text": text}]})),
            other => Err((-32602, format!("no resource {other:?}"))),
        },
        "prompts/list" => Ok(json!({"prompts": [{
            "name": "genome", "description": "The genome of this session, as it starts.",
        }]})),
        "prompts/get" => match params["name"].as_str() {
            Some("genome") => genome(ctx, session).map(|text| {
                json!({"description": "The genome of this session.",
                    "messages": [{"role": "user", "content": {"type": "text", "text": text}}]})
            }),
            other => Err((-32602, format!("no prompt {other:?}"))),
        },
        "tools/list" => Ok(json!({"tools": [{
            "name": "expression",
            "description": "What a turn needs: the acting persona and the work's record, the first rules of the bring-ins that fit a prompt in a folder, and the memories that bear on it.",
            "inputSchema": {"type": "object", "properties": {
                "prompt": {"type": "string", "description": "what the turn is about"},
                "cwd": {"type": "string", "description": "the folder, absolute, under this server's own; its own when left out"},
            }, "required": ["prompt"]},
        }]})),
        "tools/call" => match params["name"].as_str() {
            Some("expression") => {
                let args = &params["arguments"];
                // A folder under the server's own, links and `..` resolved,
                // never one elsewhere a prompt might name.
                let own = session.cwd.as_ref().and_then(|o| o.canonicalize().ok());
                let cwd = args["cwd"]
                    .as_str()
                    .and_then(|c| PathBuf::from(c).canonicalize().ok())
                    .filter(|c| own.as_ref().is_some_and(|o| c.starts_with(o)))
                    .or_else(|| session.cwd.clone());
                let input = json!({"prompt": args["prompt"].as_str().unwrap_or_default(), "cwd": cwd});
                run(ctx, session, entry::expression, input, Slice::Whole).map(|text| {
                    let text = match text.is_empty() {
                        true => "Nothing fits this turn.".to_string(),
                        false => text,
                    };
                    json!({"content": [{"type": "text", "text": text}]})
                })
            }
            other => Err((-32602, format!("no tool {other:?}"))),
        },
        other => Err((-32601, format!("no method {other:?}"))),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, why)) => error(id, code, &why),
    })
}

fn genome(ctx: &Ctx, session: &Session) -> Result<String, (i64, String)> {
    let input = json!({"cwd": session.cwd});
    run(ctx, session, entry::genome, input, Slice::Whole)
}

type Entry = fn(&Ctx, &Call) -> anyhow::Result<Vec<Item>>;

fn run(
    ctx: &Ctx,
    session: &Session,
    f: Entry,
    input: Value,
    slice: Slice,
) -> Result<String, (i64, String)> {
    let call = Call {
        session: session.id.clone(),
        carrier: Carrier::OnDemand,
        reset: false,
        input,
        slice,
    };
    f(ctx, &call)
        .map(|items| items.into_iter().map(|i| i.text).collect())
        .map_err(|e| (-32603, format!("{e:#}")))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
