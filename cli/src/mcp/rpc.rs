use nothing_agentapi::json::{Json, parse};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

pub enum Incoming {
    Request {
        id: Json,
        method: String,
        params: Json,
    },
    Notification,
    Reply,
    Malformed(String),
    Invalid {
        id: Json,
        message: String,
    },
}

pub fn read_message(line: &str) -> Option<Incoming> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let message = match parse(trimmed) {
        Ok(value) => value,
        Err(error) => return Some(Incoming::Malformed(error.to_string())),
    };
    if !matches!(message, Json::Obj(_)) {
        return Some(Incoming::Invalid {
            id: Json::Null,
            message: "a JSON-RPC message must be a single JSON object on one line".to_string(),
        });
    }

    let id = message.get("id").cloned();
    let method = message
        .get("method")
        .and_then(Json::as_str)
        .map(str::to_string);
    let params = message
        .get("params")
        .cloned()
        .unwrap_or_else(|| Json::Obj(Vec::new()));

    match (id, method) {
        (Some(id), Some(method)) => Some(Incoming::Request { id, method, params }),
        (None, Some(_)) => Some(Incoming::Notification),
        (Some(id), None) => {
            if message.get("result").is_some() || message.get("error").is_some() {
                Some(Incoming::Reply)
            } else {
                Some(Incoming::Invalid {
                    id,
                    message: "a JSON-RPC request needs a `method` string".to_string(),
                })
            }
        }
        (None, None) => Some(Incoming::Invalid {
            id: Json::Null,
            message: "a JSON-RPC message needs a `method` string".to_string(),
        }),
    }
}

pub fn success(id: &Json, result: Json) -> Json {
    Json::Obj(vec![
        ("jsonrpc".to_string(), Json::str("2.0")),
        ("id".to_string(), id.clone()),
        ("result".to_string(), result),
    ])
}

pub fn failure(id: &Json, code: i64, message: impl Into<String>) -> Json {
    Json::Obj(vec![
        ("jsonrpc".to_string(), Json::str("2.0")),
        ("id".to_string(), id.clone()),
        (
            "error".to_string(),
            Json::obj(vec![
                ("code", Json::Int(code)),
                ("message", Json::str(message.into())),
            ]),
        ),
    ])
}
