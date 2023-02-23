use clap::{Arg, ArgMatches, Command};
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::process::exit;
use std::thread;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Endpoint {
    status_message: Option<String>,
    grade: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Response {
    host: String,
    status: String,
    status_message: Option<String>,
    endpoints: Option<Vec<Endpoint>>,
}

#[derive(Debug)]
struct Status {
    ready: bool,
    status: String,
    grade: String,
    message: String,
    code: i32,
}

fn main() {
    let cli_options = get_cli_parameters();
    let mut status = Status {
        ready: false,
        status: "".to_string(),
        grade: "".to_string(),
        message: "".to_string(),
        code: 0,
    };

    let mut count = 0;
    while status.ready == false {
        count += 1;
        // println!("Count: {}", count);
        let mut body = "".to_string();

        if let Some(url) = cli_options.get_one::<String>("url") {
            body = get_url_body(url.to_string());
        } else {
        }

        status = process_response_body(body, status);

        if status.ready == false {
            let pause_duration = Duration::from_secs(10);
            thread::sleep(pause_duration);
        }
        if count > 5 {
            break;
        }
    }

    print_result(&status);
    exit(status.code);
}

fn get_url_body(url: String) -> String {
    let request_url = format!(
        "{}{}{}",
        "https://api.ssllabs.com/api/v3/analyze?host=", url, "&publish=off&fromCache=on"
    );
    let response = get(request_url).unwrap();

    // Capture the response body
    let body = response.text().unwrap();
    // println!("{}", body);
    body
}

fn get_cli_parameters() -> ArgMatches {
    let matches = Command::new("qualysapp")
        .arg(
            Arg::new("url")
                .help("URL address")
                .short('u')
                .required(false),
        )
        .arg(
            Arg::new("time")
                .help("Time to pause between request attempts to the API")
                .short('t')
                .required(false),
        )
        .get_matches();

    return matches;
}

fn process_response_body(body: String, mut status: Status) -> Status {
    let response: Response = serde_json::from_str(&body).unwrap();
    // i.e. Unable to resolve domain name
    status = set_response_status(status, &response);
    // Continue otherwise
    match response.endpoints {
        Some(endpoints) => {
            if let Some(endpoint) = endpoints.first() {
                let grade = endpoint
                    .grade
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or_default();
                let status_message = endpoint
                    .status_message
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or_default();
                status = set_status(status, status_message);
                status.grade = grade.to_string();
                status = set_exit_code(status);
            } else {
                status.code = 3;
                status.status = "ERROR".to_string();
                status.message = "Endpoint not ready".to_string();
            }
        }
        None => {
            status.code = 3;
            status.status = "ERROR".to_string();
            status.message = "No endpoint".to_string();
        }
    }
    status
}

fn set_response_status(mut status: Status, response: &Response) -> Status {
    match response.status.as_str() {
        "DNS" => status.status = "DNS".to_string(),
        "ERROR" => status.status = "ERROR".to_string(),
        "IN_PROGRESS" => status.status = "IN_PROGRESS".to_string(),
        "READY" => status.status = "OK".to_string(),
        _ => status.status = "UNKOWN".to_string(),
    }

    if status.status == "ERROR" {
        status.message = response
            .status_message
            .as_ref()
            .map(String::as_str)
            .unwrap_or_default()
            .to_string();
        status.code = 3;
        status.ready = true;
        return status;
    } else if status.status == "IN_PROGRESS" || status.status == "READY" || status.status == "DNS" {
        status.code = 0;
        status.ready = false;
        return status;
    } else {
        status.message = response
            .status_message
            .as_ref()
            .map(String::as_str)
            .unwrap_or_default()
            .to_string();
        status.code = 3;
        status.ready = true;
        return status;
    }
}

fn set_status(mut status: Status, status_message: &str) -> Status {
    status.status = status_message.to_string();
    if status.status == "Ready" {
        status.ready = true;
    }
    status
}

fn set_exit_code(mut status: Status) -> Status {
    match status.grade.as_str() {
        "A" | "A+" => status.code = 0,
        "A-" => status.code = 1,
        "B" | "C" | "D" | "E" | "F" | "M" | "T" => status.code = 2,
        _ => status.code = 2,
    }
    status
}

fn print_result(status: &Status) {
    if status.grade.is_empty() && !status.message.is_empty() {
        println!("{}: {}", status.status, status.message);
    } else if !status.grade.is_empty() && status.message.is_empty() {
        println!("{}", status.grade);
    } else {
        println!("{} {}: {}", status.grade, status.status, status.message);
    }
    // println!("Print all anyway: {} {}", status.grade, status.message);
    // println!("{:?}", status);
}
