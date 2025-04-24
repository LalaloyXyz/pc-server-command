use warp::Filter;
use std::process::Command;
use local_ip_address::local_ip;
use serde::Deserialize;
use warp::http::StatusCode;

#[derive(Deserialize)]
struct CommandRequest {
    command: String,
}

#[tokio::main]
async fn main() {
    let ip = local_ip().unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
    let port = 8080;

    println!("Server running at: http://{}:{}", ip, port);
    println!("To send a command, use:");
    println!(
        "curl -X POST http://{}:{}/ -H \"Content-Type: application/json\" -d '{{\"command\": \"open (you want to open app)\"}}'",
        ip, port
    );

    let post_route = warp::post()
        .and(warp::path::end())
        .and(warp::body::json())
        .map(handle_command);

    warp::serve(post_route)
        .run((ip, port))
        .await;
}

fn handle_command(body: CommandRequest) -> impl warp::Reply {
    let command = body.command.trim().to_lowercase();
    println!("Received command: {}", command);

    let open_prefixes = ["open ", "เปิด ", "open", "เปิด"];
    let search_prefixes = ["search ", "ค้นหา ", "search", "ค้นหา"];

    let response = match parse_prefix(&command, &open_prefixes) {
        Some(input) => try_launch_app(&input),
        None => match parse_prefix(&command, &search_prefixes) {
            Some(input) => search_for_app(&input),
            None => "Invalid command.".to_string(),
        },
    };

    warp::reply::with_status(response, StatusCode::OK)
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
