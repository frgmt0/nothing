use std::io::Write;
use std::process::{Command, Stdio};

pub const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";
pub const DEFAULT_BIN: &str = "claude";

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Claude {
    pub bin: String,
    pub model: String,
    pub retries: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reply {
    pub text: String,
    pub attempts: usize,
}

impl Default for Claude {
    fn default() -> Claude {
        Claude {
            bin: std::env::var("NOTHING_CLAUDE_BIN").unwrap_or_else(|_| DEFAULT_BIN.to_string()),
            model: std::env::var("NOTHING_CLAUDE_MODEL")
                .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            retries: 1,
        }
    }
}

impl Claude {
    pub fn new() -> Claude {
        Claude::default()
    }

    pub fn ask(&self, prompt: &str) -> Result<Reply, String> {
        let mut last = String::new();
        for attempt in 0..=self.retries {
            match self.once(prompt) {
                Ok(text) if !text.trim().is_empty() => {
                    return Ok(Reply {
                        text,
                        attempts: attempt + 1,
                    });
                }
                Ok(_) => last = "the model returned an empty reply".to_string(),
                Err(message) => last = message,
            }
        }
        Err(last)
    }

    fn once(&self, prompt: &str) -> Result<String, String> {
        let mut child = Command::new(&self.bin)
            .arg("-p")
            .arg("--model")
            .arg(&self.model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run `{}`: {e}", self.bin))?;

        child
            .stdin
            .as_mut()
            .ok_or_else(|| "no stdin on the claude process".to_string())?
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("cannot write the prompt: {e}"))?;
        drop(child.stdin.take());

        let output = child
            .wait_with_output()
            .map_err(|e| format!("cannot read the reply: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "claude exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

pub fn first_meaningful_line(reply: &str) -> String {
    for line in reply.lines() {
        let line = line
            .trim()
            .trim_start_matches('`')
            .trim_end_matches('`')
            .trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        return line.to_string();
    }
    reply.trim().to_string()
}

pub fn action_lines(reply: &str) -> Vec<String> {
    let body = crate::measure::text_parse::strip_fences(reply);
    body.lines()
        .map(|line| {
            line.trim()
                .trim_start_matches('`')
                .trim_end_matches('`')
                .trim()
        })
        .map(|line| match line.find('#') {
            Some(i) => line[..i].trim(),
            None => line,
        })
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_meaningful_line_survives_backticks_and_blank_lines() {
        assert_eq!(
            first_meaningful_line("\n\n`construct-num 3`\n"),
            "construct-num 3"
        );
        assert_eq!(first_meaningful_line("construct-lam"), "construct-lam");
    }

    #[test]
    fn action_lines_strip_fences_and_comments() {
        let reply =
            "```\nconstruct-num 1  # the left operand\nconstruct-binop add\n\nconstruct-num 2\n```";
        assert_eq!(
            action_lines(reply),
            vec!["construct-num 1", "construct-binop add", "construct-num 2"]
        );
    }

    #[test]
    fn the_default_model_is_the_one_the_benchmark_reports() {
        let claude = Claude {
            bin: DEFAULT_BIN.to_string(),
            model: DEFAULT_MODEL.to_string(),
            retries: 1,
        };
        assert_eq!(claude.model, "claude-haiku-4-5-20251001");
    }
}
