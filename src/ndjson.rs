//! NDJSON framing: one JSON object per line.

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

pub async fn read_json_line<R>(reader: &mut R) -> Result<Option<Value>>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .await
        .context("reading NDJSON line")?;
    if n == 0 {
        return Ok(None);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_json::from_str(trimmed).context("parsing NDJSON line")?;
    Ok(Some(value))
}

pub async fn write_json_line<W>(writer: &mut W, value: &Value) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let mut bytes = serde_json::to_vec(value).context("serializing NDJSON")?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await.context("writing NDJSON")?;
    Ok(())
}

pub async fn write_json_line_flush<W>(writer: &mut W, value: &Value) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    write_json_line(writer, value).await?;
    writer.flush().await.context("flushing NDJSON")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn read_json_line_parses_object() {
        let mut reader = BufReader::new(b"{\"a\":1}\n".as_slice());
        let v = read_json_line(&mut reader).await.unwrap().unwrap();
        assert_eq!(v["a"], 1);
    }

    #[tokio::test]
    async fn write_json_line_appends_newline() {
        let mut buf = Vec::new();
        write_json_line(&mut buf, &serde_json::json!({"ok": true}))
            .await
            .unwrap();
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "{\"ok\":true}\n");
    }

    #[tokio::test]
    async fn read_json_line_eof_returns_none() {
        let mut reader = BufReader::new(b"".as_slice());
        assert!(read_json_line(&mut reader).await.unwrap().is_none());
    }
}
