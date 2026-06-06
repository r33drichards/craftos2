//! craftos-mcp — an MCP server that drives an embedded CraftOS-PC to run
//! declarative, simulated multi-computer CC tests (rednet / GPS / ...).
//!
//! Transports: stdio (default), streamable-http, and legacy SSE — selected with
//! `--transport`. The embedded emulator writes noise to the process stdout, so
//! for stdio we hand rmcp the *real* stdout and redirect the emulator's fd 1 to
//! a log file, keeping the JSON-RPC channel clean.

use std::ffi::CString;
use std::os::raw::c_char;
use std::os::unix::io::FromRawFd;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};

extern "C" {
    fn cc_gps_selftest(rom: *const c_char) -> i32;
}

fn rom_path() -> String {
    std::env::var("CRAFTOS_ROM").unwrap_or_else(|_| {
        format!("{}/craftos2-rom", std::env::var("HOME").unwrap_or_default())
    })
}

#[derive(Clone)]
struct CraftosMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl CraftosMcp {
    fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    #[tool(
        description = "Run the embedded multi-computer GPS self-test: boot 4 \
        wireless GPS hosts at known coordinates plus a client, and verify \
        gps.locate() trilaterates the client's true position (3,4,5). Returns \
        JSON {pass, expected, actual}."
    )]
    async fn gps_selftest(&self) -> String {
        let rom = rom_path();
        let result = tokio::task::spawn_blocking(move || {
            let c = CString::new(rom).unwrap();
            unsafe { cc_gps_selftest(c.as_ptr()) }
        })
        .await
        .unwrap_or(-1);
        let pass = result == 1;
        format!(
            "{{\"pass\":{pass},\"expected\":\"3,4,5\",\"detail\":\"{}\"}}",
            if pass { "client resolved its position via GPS" } else { "GPS did not resolve" }
        )
    }
}

#[tool_handler]
impl ServerHandler for CraftosMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Drives an embedded CraftOS-PC emulator to run simulated \
             multi-computer ComputerCraft tests (rednet, GPS). Call \
             gps_selftest to verify the GPS simulation end to end."
                .into(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

/// Redirect the emulator's stdout (fd 1) to a log file, returning a tokio handle
/// to the *original* stdout for rmcp's JSON-RPC channel.
fn detach_stdout(log: &str) -> tokio::fs::File {
    use std::os::unix::io::AsRawFd;
    let real = unsafe { libc::dup(1) };
    let logfile = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .expect("open log");
    unsafe {
        libc::dup2(logfile.as_raw_fd(), 1);
    }
    let real_std = unsafe { std::fs::File::from_raw_fd(real) };
    tokio::fs::File::from_std(real_std)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let transport = std::env::args()
        .skip_while(|a| a != "--transport")
        .nth(1)
        .unwrap_or_else(|| "stdio".into());

    match transport.as_str() {
        "stdio" => {
            // rmcp speaks JSON-RPC on the real stdout; emulator noise -> log.
            let out = detach_stdout("/tmp/craftos-mcp.log");
            let service = CraftosMcp::new()
                .serve((tokio::io::stdin(), out))
                .await?;
            service.waiting().await?;
        }
        other => {
            anyhow::bail!("transport '{other}' not wired yet (stdio works; http/sse next)");
        }
    }
    Ok(())
}
