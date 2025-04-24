use warp::Filter;
use std::process::Command;
use local_ip_address::local_ip;
use serde::Deserialize;
use warp::http::StatusCode;
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};
use tokio::time::sleep;

const WAIT_FINAL_TIME: u64 = 1500;

#[derive(Deserialize)]
struct CommandRequest {
    command: String,
}

#[derive(Clone)]
struct SharedState {
    last_command: Arc<Mutex<(String, Instant)>>,
}

#[tokio::main]
async fn main() {
    let ip = local_ip().unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
    let port = 8080;

    let state = SharedState {
        last_command: Arc::new(Mutex::new(("".to_string(), Instant::now()))),
    };

    let state_filter = warp::any().map(move || state.clone());

    println!("Local IP : http://{}:{}", ip, port);
    println!(
        "curl -X POST http://{}:{}/ -H \"Content-Type: application/json\" -d '{{\"command\": \"open google\"}}'",
        ip, port
    );

    let post_route = warp::post()
        .and(warp::path::end())
        .and(warp::body::json())
        .and(state_filter)
        .map(handle_command);

    warp::serve(post_route)
        .run((ip, port))
        .await;
}

fn handle_command(body: CommandRequest, state: SharedState) -> impl warp::Reply {
    let now = Instant::now();
    let command = body.command.trim().to_lowercase();
    {
        let mut lock = state.last_command.lock().unwrap();
        *lock = (command.clone(), now);
    }

    let shared = state.clone();
    tokio::spawn(async move {
        sleep(Duration::from_millis(WAIT_FINAL_TIME)).await;
        let (cmd, timestamp) = {
            let lock = shared.last_command.lock().unwrap();
            lock.clone()
        };

        if timestamp == now {
            println!("Final command : {}", cmd);

            let open_prefixes = ["open ", "เปิด ", "open", "เปิด"];
            let search_prefixes = ["search ", "ค้นหา ", "search", "ค้นหา"];

            let response = match parse_prefix(&cmd, &open_prefixes) {
                Some(input) => try_launch_app(&input),
                None => match parse_prefix(&cmd, &search_prefixes) {
                    Some(input) => search_for_app(&input),
                    None => "Invalid command.".to_string(),
                },
            };

            println!("Response: {}", response);
        } else {
            println!("Skipped outdated command: {}", cmd);
        }
    });

    warp::reply::with_status("Waiting for final input...", StatusCode::ACCEPTED)
}

fn parse_prefix(command: &str, prefixes: &[&str]) -> Option<String> {
    prefixes.iter().find_map(|p| {
        command.strip_prefix(p).map(|s| s.trim().to_string())
    })
}

fn try_launch_app(app_name: &str) -> String {
    let normalized_name = app_name.replace(' ', "");
    match launch_app(&normalized_name) {
        Ok(msg) => msg,
        Err(_) => open_for_app(&normalized_name),
    }
}

fn launch_app(app_name: &str) -> Result<String, std::io::Error> {
    #[cfg(target_os = "linux")]
    {
        if Command::new(app_name).spawn()?.wait()?.success() {
            return Ok(format!("Launching app: {}", app_name));
        }

        if Command::new("gtk-launch").arg(app_name).status()?.success() {
            return Ok(format!("Launching via gtk-launch: {}", app_name));
        }

        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to launch"))
    }

    #[cfg(target_os = "macos")]
    {
        if Command::new("open").arg("-a").arg(app_name).status()?.success() {
            return Ok(format!("Launching app: {}", app_name));
        }

        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to launch"))
    }

    #[cfg(target_os = "windows")]
    {
        if Command::new("cmd").args(["/C", "start", "", app_name]).status()?.success() {
            return Ok(format!("Launching app: {}", app_name));
        }

        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to launch"))
    }
}

fn open_for_app(input: &str) -> String {
    open_url(&format!("https://{}.com/", input), &format!("App not found. Opening: {}", input))
}

fn search_for_app(input: &str) -> String {
    let url = format!("https://www.google.com/search?q={}", input.replace(' ', "+"));
    open_url(&url, &format!("Searching for: {}", input))
}

fn open_url(url: &str, success_msg: &str) -> String {
    let command = if cfg!(target_os = "windows") {
        vec!["cmd", "/C", "start", "", url]
    } else if cfg!(target_os = "macos") {
        vec!["open", url]
    } else {
        vec!["xdg-open", url]
    };

    match Command::new(command[0]).args(&command[1..]).status() {
        Ok(status) if status.success() => success_msg.to_string(),
        _ => format!("Failed to launch or open: {}", url),
    }
}
