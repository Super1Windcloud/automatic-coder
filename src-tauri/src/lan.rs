use crate::{app_error, app_info, app_warn};
use if_addrs::{IfAddr, get_if_addrs};
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{App, Manager, Wry};

const LAN_SERVER_PORT_START: u16 = 37999;
const LAN_SERVER_PORT_END: u16 = 38009;
const READ_TIMEOUT: Duration = Duration::from_millis(600);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Default, Clone)]
pub struct LanAnswerState {
    snapshot: Arc<RwLock<AnswerSnapshot>>,
    port: Arc<Mutex<Option<u16>>>,
}

#[derive(Default, Clone, Serialize)]
struct AnswerSnapshot {
    content: String,
    updated_at_ms: u128,
}

pub fn start_lan_answer_server(app: &mut App<Wry>) -> Result<(), String> {
    let state: tauri::State<LanAnswerState> = app.state();
    let snapshot = state.snapshot.clone();
    let port_holder = state.port.clone();

    let mut listener = None;
    let mut selected_port = None;
    for port in LAN_SERVER_PORT_START..=LAN_SERVER_PORT_END {
        match TcpListener::bind(("0.0.0.0", port)) {
            Ok(bound) => {
                listener = Some(bound);
                selected_port = Some(port);
                break;
            }
            Err(err) => {
                app_warn!("lan", "failed to bind 0.0.0.0:{port}: {err}");
            }
        }
    }

    let Some(listener) = listener else {
        return Err(format!(
            "局域网答案服务启动失败，端口范围 {}-{} 均不可用",
            LAN_SERVER_PORT_START, LAN_SERVER_PORT_END
        ));
    };

    let port = selected_port.expect("selected port must exist when listener exists");
    *port_holder
        .lock()
        .map_err(|_| "局域网服务端口锁获取失败".to_string())? = Some(port);

    let lan_urls = collect_lan_urls(port);
    for url in &lan_urls {
        app_info!("lan", "LAN answer viewer available at {url}");
    }

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let snapshot = snapshot.clone();
                    thread::spawn(move || {
                        if let Err(err) = handle_connection(stream, snapshot) {
                            app_warn!("lan", "request handling failed: {err}");
                        }
                    });
                }
                Err(err) => {
                    app_error!("lan", "incoming connection failed: {err}");
                }
            }
        }
    });

    Ok(())
}

pub fn clear_answer_snapshot(app_handle: &tauri::AppHandle) {
    set_answer_snapshot(app_handle, String::new());
}

pub fn append_answer_snapshot(app_handle: &tauri::AppHandle, chunk: &str) {
    if chunk.is_empty() {
        return;
    }

    let state = app_handle.state::<LanAnswerState>();
    if let Ok(mut snapshot) = state.snapshot.write() {
        snapshot.content.push_str(chunk);
        snapshot.updated_at_ms = current_timestamp_ms();
    }
}

pub fn set_answer_snapshot(app_handle: &tauri::AppHandle, content: String) {
    let state = app_handle.state::<LanAnswerState>();
    if let Ok(mut snapshot) = state.snapshot.write() {
        snapshot.content = content;
        snapshot.updated_at_ms = current_timestamp_ms();
    }
}

pub fn current_lan_urls(app_handle: &tauri::AppHandle) -> Vec<String> {
    let state = app_handle.state::<LanAnswerState>();
    let Some(port) = state.port.lock().ok().and_then(|port| *port) else {
        return Vec::new();
    };
    collect_lan_urls(port)
}

fn handle_connection(
    mut stream: TcpStream,
    snapshot: Arc<RwLock<AnswerSnapshot>>,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|err| format!("failed to set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(WRITE_TIMEOUT))
        .map_err(|err| format!("failed to set write timeout: {err}"))?;

    let mut buffer = [0_u8; 8192];
    let size = stream
        .read(&mut buffer)
        .map_err(|err| format!("failed to read request: {err}"))?;
    if size == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/");

    if method != "GET" {
        write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
        )?;
        return Ok(());
    }

    match path {
        "/" => write_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            ANSWER_VIEWER_HTML.as_bytes(),
        )?,
        "/api/state" => {
            let body = {
                let snapshot = snapshot
                    .read()
                    .map_err(|_| "局域网答案快照锁获取失败".to_string())?;
                serde_json::to_vec(&*snapshot)
                    .map_err(|err| format!("failed to encode state json: {err}"))?
            };
            write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            )?;
        }
        "/health" => write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", b"ok")?,
        "/favicon.ico" => write_response(&mut stream, "204 No Content", "image/x-icon", &[])?,
        _ => write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
        )?,
    }

    Ok(())
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("failed to write response header: {err}"))?;
    stream
        .write_all(body)
        .map_err(|err| format!("failed to write response body: {err}"))?;
    stream
        .flush()
        .map_err(|err| format!("failed to flush response: {err}"))?;
    Ok(())
}

fn current_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn collect_lan_urls(port: u16) -> Vec<String> {
    let mut urls = vec![format!("http://127.0.0.1:{port}")];
    if let Some(ip) = detect_local_ip() {
        urls.push(format!("http://{}:{port}", ip));
    }
    urls
}

fn detect_local_ip() -> Option<IpAddr> {
    let mut candidates = get_if_addrs()
        .ok()?
        .into_iter()
        .filter_map(|iface| {
            let IfAddr::V4(addr) = iface.addr else {
                return None;
            };

            if !is_preferred_lan_ipv4(addr.ip) {
                return None;
            }

            Some((score_interface(&iface.name, addr.ip), IpAddr::V4(addr.ip)))
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    candidates.into_iter().map(|(_, ip)| ip).next()
}

fn is_preferred_lan_ipv4(ip: Ipv4Addr) -> bool {
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast() {
        return false;
    }

    let [a, b, ..] = ip.octets();
    if a == 198 && (18..=19).contains(&b) {
        return false;
    }

    ip.is_private()
}

fn score_interface(name: &str, ip: Ipv4Addr) -> u8 {
    let mut score = private_range_score(ip);
    let lower = name.to_ascii_lowercase();

    if lower.contains("wlan")
        || lower.contains("wi-fi")
        || lower.contains("wifi")
        || lower.contains("wireless")
        || lower.contains("无线")
    {
        score = score.saturating_add(30);
    }

    if lower.contains("ethernet") || lower.contains("以太网") {
        score = score.saturating_add(20);
    }

    if lower.contains("mihomo")
        || lower.contains("vpn")
        || lower.contains("virtual")
        || lower.contains("tun")
        || lower.contains("tap")
        || lower.contains("loopback")
    {
        score = score.saturating_sub(40);
    }

    score
}

fn private_range_score(ip: Ipv4Addr) -> u8 {
    let [a, b, ..] = ip.octets();
    match (a, b) {
        (192, 168) => 90,
        (10, _) => 80,
        (172, 16..=31) => 70,
        _ => 10,
    }
}

const ANSWER_VIEWER_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Interview Coder LAN Viewer</title>
    <style>
      :root {
        color-scheme: light;
        --bg: #f5efe6;
        --panel: rgba(255, 252, 247, 0.92);
        --ink: #1e2430;
        --muted: #6a7485;
        --line: rgba(30, 36, 48, 0.12);
        --accent: #d46a4c;
        --accent-soft: rgba(212, 106, 76, 0.12);
        --code-bg: #17202a;
        --code-ink: #f7f8fa;
      }

      * { box-sizing: border-box; }

      body {
        margin: 0;
        padding: 0;
        font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
        color: var(--ink);
        background:
          radial-gradient(circle at top left, rgba(212, 106, 76, 0.22), transparent 28%),
          radial-gradient(circle at right 20%, rgba(35, 88, 132, 0.16), transparent 24%),
          linear-gradient(160deg, #f8f2ea 0%, #efe5d8 48%, #e9ddd0 100%);
        min-height: 100vh;
        overflow-x: hidden;
      }

      .shell {
        width: min(980px, calc(100vw - 32px));
        margin: 24px auto;
        padding: 20px;
        border: 1px solid var(--line);
        border-radius: 24px;
        background: var(--panel);
        backdrop-filter: blur(10px);
        box-shadow: 0 18px 60px rgba(49, 45, 38, 0.12);
      }

      .header {
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
        gap: 16px;
        margin-bottom: 20px;
      }

      .title {
        margin: 0;
        font-size: 28px;
        line-height: 1.1;
      }

      .subtitle {
        margin: 8px 0 0;
        color: var(--muted);
        font-size: 14px;
      }

      .status {
        min-width: 150px;
        padding: 10px 14px;
        border-radius: 14px;
        background: var(--accent-soft);
        color: #8c3d28;
        font-size: 13px;
        line-height: 1.5;
      }

      .content {
        min-height: 60vh;
        padding: 22px;
        border-radius: 18px;
        border: 1px solid var(--line);
        background: rgba(255, 255, 255, 0.72);
      }

      .empty {
        display: grid;
        place-items: center;
        min-height: 46vh;
        color: var(--muted);
        text-align: center;
        letter-spacing: 0.02em;
      }

      h1, h2, h3, h4 {
        margin: 1.2em 0 0.55em;
        line-height: 1.25;
      }

      h1:first-child, h2:first-child, h3:first-child, h4:first-child, p:first-child {
        margin-top: 0;
      }

      p, ul, ol {
        margin: 0 0 0.9em;
        line-height: 1.75;
      }

      ul, ol {
        padding-left: 1.4em;
      }

      li {
        margin: 0.3em 0;
      }

      pre {
        margin: 1em 0;
        padding: 16px;
        overflow: auto;
        max-width: 100%;
        border-radius: 16px;
        background: var(--code-bg);
        color: var(--code-ink);
      }

      code {
        font-family: "JetBrains Mono", "Cascadia Code", Consolas, monospace;
      }

      :not(pre) > code {
        padding: 0.15em 0.4em;
        border-radius: 8px;
        background: rgba(23, 32, 42, 0.08);
      }

      blockquote {
        margin: 1em 0;
        padding: 0.2em 0 0.2em 1em;
        border-left: 4px solid rgba(35, 88, 132, 0.35);
        color: #435163;
      }

      img, table {
        max-width: 100%;
      }

      @media (max-width: 720px) {
        .shell {
          width: min(100vw - 12px, 100%);
          margin: 6px auto;
          padding: 12px;
          border-radius: 18px;
        }

        .header {
          flex-direction: column;
          gap: 12px;
        }

        .title {
          font-size: 22px;
          line-height: 1.2;
        }

        .subtitle {
          font-size: 13px;
          line-height: 1.6;
        }

        .status {
          width: 100%;
          min-width: 0;
        }

        .content {
          min-height: 52vh;
          padding: 14px;
          border-radius: 16px;
        }

        p, ul, ol {
          line-height: 1.7;
        }

        pre {
          padding: 12px;
          border-radius: 14px;
          font-size: 12px;
        }

        :not(pre) > code {
          overflow-wrap: anywhere;
        }
      }

      @media (max-width: 420px) {
        body {
          background:
            radial-gradient(circle at top left, rgba(212, 106, 76, 0.2), transparent 36%),
            linear-gradient(160deg, #f8f2ea 0%, #efe5d8 52%, #e9ddd0 100%);
        }

        .shell {
          width: min(100vw - 8px, 100%);
          margin: 4px auto;
          padding: 10px;
          border-radius: 14px;
        }

        .title {
          font-size: 20px;
        }

        .content {
          padding: 12px;
        }
      }
    </style>
  </head>
  <body>
    <main class="shell">
      <section class="header">
        <div>
          <h1 class="title">Interview Coder 局域网答案页</h1>
          <p class="subtitle">应用启动后会自动同步最新流式答案，这个页面每 350ms 刷新一次。</p>
        </div>
        <div class="status">
          <div id="status-text">等待连接</div>
          <div id="status-time">尚未收到答案</div>
        </div>
      </section>
      <section id="content" class="content">
        <div class="empty">等待新的答案生成...</div>
      </section>
    </main>
    <script>
      const contentEl = document.getElementById('content');
      const statusTextEl = document.getElementById('status-text');
      const statusTimeEl = document.getElementById('status-time');
      let lastSignature = '';

      function escapeHtml(value) {
        return value
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;');
      }

      function renderInline(text) {
        let html = escapeHtml(text);
        html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
        html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
        html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');
        return html;
      }

      function markdownToHtml(markdown) {
        const lines = markdown.replace(/\r\n/g, '\n').split('\n');
        const chunks = [];
        let paragraph = [];
        let listType = '';
        let listItems = [];
        let inCode = false;
        let codeLines = [];

        function flushParagraph() {
          if (!paragraph.length) return;
          chunks.push('<p>' + renderInline(paragraph.join('<br />')) + '</p>');
          paragraph = [];
        }

        function flushList() {
          if (!listItems.length) return;
          const tag = listType === 'ol' ? 'ol' : 'ul';
          chunks.push('<' + tag + '>' + listItems.map((item) => '<li>' + renderInline(item) + '</li>').join('') + '</' + tag + '>');
          listItems = [];
          listType = '';
        }

        function flushCode() {
          if (!inCode) return;
          chunks.push('<pre><code>' + escapeHtml(codeLines.join('\n')) + '</code></pre>');
          inCode = false;
          codeLines = [];
        }

        for (const line of lines) {
          if (line.startsWith('```')) {
            flushParagraph();
            flushList();
            if (inCode) {
              flushCode();
            } else {
              inCode = true;
            }
            continue;
          }

          if (inCode) {
            codeLines.push(line);
            continue;
          }

          if (!line.trim()) {
            flushParagraph();
            flushList();
            continue;
          }

          const heading = line.match(/^(#{1,4})\s+(.*)$/);
          if (heading) {
            flushParagraph();
            flushList();
            const level = heading[1].length;
            chunks.push('<h' + level + '>' + renderInline(heading[2]) + '</h' + level + '>');
            continue;
          }

          const quote = line.match(/^>\s?(.*)$/);
          if (quote) {
            flushParagraph();
            flushList();
            chunks.push('<blockquote>' + renderInline(quote[1]) + '</blockquote>');
            continue;
          }

          const ordered = line.match(/^\d+\.\s+(.*)$/);
          if (ordered) {
            flushParagraph();
            if (listType && listType !== 'ol') flushList();
            listType = 'ol';
            listItems.push(ordered[1]);
            continue;
          }

          const unordered = line.match(/^[-*]\s+(.*)$/);
          if (unordered) {
            flushParagraph();
            if (listType && listType !== 'ul') flushList();
            listType = 'ul';
            listItems.push(unordered[1]);
            continue;
          }

          flushList();
          paragraph.push(line);
        }

        flushParagraph();
        flushList();
        flushCode();

        return chunks.join('');
      }

      async function loadState() {
        try {
          const res = await fetch('/api/state', { cache: 'no-store' });
          if (!res.ok) throw new Error('HTTP ' + res.status);
          const data = await res.json();
          const signature = String(data.updated_at_ms || 0) + '::' + (data.content || '').length;

          statusTextEl.textContent = data.content ? '同步中' : '等待答案';
          statusTimeEl.textContent = data.updated_at_ms
            ? '更新于 ' + new Date(Number(data.updated_at_ms)).toLocaleString()
            : '尚未收到答案';

          if (signature === lastSignature) return;
          lastSignature = signature;

          if (!data.content) {
            contentEl.innerHTML = '<div class="empty">等待新的答案生成...</div>';
            return;
          }

          contentEl.innerHTML = markdownToHtml(data.content);
        } catch (error) {
          statusTextEl.textContent = '连接中断';
          statusTimeEl.textContent = '无法读取本地答案服务';
        }
      }

      loadState();
      window.setInterval(loadState, 350);
    </script>
  </body>
</html>
"#;
