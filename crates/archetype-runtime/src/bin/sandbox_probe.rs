use std::{
    env, fs, io,
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::{Command, ExitCode},
    time::Duration,
};

use serde_json::{Value, json};

const NETWORK_TIMEOUT: Duration = Duration::from_millis(500);

struct ProbeResult {
    allowed: bool,
    permission_denied: bool,
    detail: String,
}

impl ProbeResult {
    fn from_io<T>(result: io::Result<T>) -> Self {
        match result {
            Ok(_) => Self {
                allowed: true,
                permission_denied: false,
                detail: "allowed".to_owned(),
            },
            Err(error) => Self {
                allowed: false,
                permission_denied: error.kind() == io::ErrorKind::PermissionDenied,
                detail: error.to_string(),
            },
        }
    }

    fn as_json(&self) -> Value {
        json!({
            "allowed": self.allowed,
            "permission_denied": self.permission_denied,
            "detail": self.detail,
        })
    }
}

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(secret_path) = arguments.next() else {
        eprintln!(
            "usage: archetype-sandbox-probe <secret-path> <external-address> [--expect-blocked]"
        );
        return ExitCode::from(2);
    };
    let Some(external_address) = arguments.next() else {
        eprintln!("missing external address");
        return ExitCode::from(2);
    };
    let expect_blocked = arguments.next().as_deref() == Some("--expect-blocked");

    let file_read = ProbeResult::from_io(fs::read(Path::new(&secret_path)));
    let listener = TcpListener::bind("127.0.0.1:0");
    let listener_result =
        ProbeResult::from_io(listener.as_ref().map(|_| ()).map_err(clone_io_error));
    let loopback_connect = ProbeResult::from_io(
        listener
            .as_ref()
            .map_err(clone_io_error)
            .and_then(|listener| {
                TcpStream::connect_timeout(&listener.local_addr()?, NETWORK_TIMEOUT)
            }),
    );
    let external_connect = external_address
        .parse::<SocketAddr>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
        .and_then(|address| TcpStream::connect_timeout(&address, NETWORK_TIMEOUT));
    let external_connect = ProbeResult::from_io(external_connect);
    let subprocess = ProbeResult::from_io(Command::new("/usr/bin/true").status());

    println!(
        "{}",
        json!({
            "file_read": file_read.as_json(),
            "loopback_connect": loopback_connect.as_json(),
            "external_connect": external_connect.as_json(),
            "listener_create": listener_result.as_json(),
            "subprocess_launch": subprocess.as_json(),
        })
    );

    let accepted = if expect_blocked {
        [
            &file_read,
            &loopback_connect,
            &external_connect,
            &listener_result,
            &subprocess,
        ]
        .into_iter()
        .all(|result| result.permission_denied)
    } else {
        [&file_read, &loopback_connect, &listener_result, &subprocess]
            .into_iter()
            .all(|result| result.allowed)
    };
    if accepted {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn clone_io_error(error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), error.to_string())
}
