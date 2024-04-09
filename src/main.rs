use std::env;
use std::process::exit;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;
use if_addrs::IfAddr;
use log::{error, info};

fn main() {
    match dotenvy::dotenv() {
        Ok(_) => info!(".env file is found and loaded"),
        Err(_) => info!(".env file is not found")
    }

    env_logger::init();
    let register_url = env::var("REGISTER_URL").expect("Cannot load var REGISTER_URL");
    let deregister_url = env::var("DEREGISTER_URL").expect("Cannot load var DEREGISTER_URL");

    let hostname = hostname::get().expect("Cannot get hostname");

    let http_client = default_http_client();

    let (sender, receiver) = channel();

    ctrlc::set_handler(move || sender.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");

    loop {
        match receiver.recv_timeout(Duration::from_secs(2)) {
            Err(RecvTimeoutError::Timeout) => {
                let ipv6 = match get_ipv6() {
                    None => { continue; }
                    Some(ip) => ip
                };

                info!("Call API /register");
                match register(&http_client, &register_url, hostname.to_str().unwrap(), &ipv6) {
                    Ok(_) => info!("Registration successfully"),
                    Err(err) => error!("Registration failed - {}", err.to_string())
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                // no point in waiting anymore :'(
                break;
            }
            Ok(_) => {
                info!("Got SIGTERM / SIGINT");
                info!("Call API /deregister");
                // deregister()
                exit(0);
            }
        }
    }
}

pub fn default_http_client() -> reqwest::blocking::Client {
    let builder = reqwest::blocking::ClientBuilder::new();
    let client = builder
        .timeout(Duration::from_secs(2))
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .expect("Failed to create custom HTTP client");
    client
}

// TODO: test this function
fn get_ipv6() -> Option<String> {
    let interfaces;
    match if_addrs::get_if_addrs() {
        Ok(data) => interfaces = data,
        Err(err) => {
            error!("{}", err);
            return None;
        }
    }

    // let ipv6_interface;
    for interface in interfaces {
        if !interface.name.contains("eth") && !interface.name.contains("enp") {
            continue;
        }

        match interface.addr {
            IfAddr::V4(_) => { continue; }
            IfAddr::V6(_) => { return Some(interface.addr.ip().to_string()); }
        }
    }

    None
}

fn register(http_client: &reqwest::blocking::Client, url: &str, hostname: &str, ipv6: &str) -> reqwest::Result<()> {
    let response = http_client.get(url)
        .send()?;
    if !response.status().is_success() {
        error!("Register failed. {:?}", response);
        // TODO: handle retry
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register() {
        let mut server = mockito::Server::new();
        let url = server.url() + "/register";

        let mock_server = server.mock("GET", "/register")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "OK"}"#)
            .expect(1)
            .create();

        let http_client = default_http_client();
        let hostname = "server1";
        let ipv6 = "2404:6800:4005:801::200e";
        let result = register(&http_client, url.as_str(), hostname, ipv6).unwrap();

        assert_eq!(result, ());
        mock_server.assert();
    }
}