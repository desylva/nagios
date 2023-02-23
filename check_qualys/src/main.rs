use clap::Parser;
use indicatif::ProgressBar;
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::process::exit;
use std::thread;
use std::time::Duration;
use addr::{DomainName};

/// Use the Qualys API to perform
/// a deep analysis of the configuration of any SSL web server on the public Internet.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Domain name to analyse
    domain: String,

    /// Pause in seconds between request attemps to the API
    #[arg(short, long, default_value_t = 15)]
    time: u8,

    /// Number of attemps to the API before giving up
    #[arg(short, long, default_value_t = 10)]
    attemps: u8,

    /// Assessment results should be published on the public results boards
    #[arg(long)]
    publish: bool,

    /// Deliver cached assessment reports when available
    #[arg(long)]
    from_cache: bool,

    /// Display a progress bar
    #[arg(long)]
    progress: bool,

    /// Make the operation more talkative
    #[arg(long)]
    verbose: bool,
}

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
    error: String,
    grade: String,
    message: String,
    exit_code: i32,
}

impl Status {
    fn new() -> Status {
        Status {
            ready: false,
            status: "".to_string(),
            grade: "".to_string(),
            message: "".to_string(),
            error: "".to_string(),
            exit_code: 0,
        }
    }

    fn set_response(&mut self, response: &Response) {
        match response.status.as_str() {
            "DNS" => self.status = "DNS".to_string(),
            "ERROR" => self.status = "ERROR".to_string(),
            "IN_PROGRESS" => self.status = "IN_PROGRESS".to_string(),
            "READY" => self.status = "OK".to_string(),
            _ => self.status = "UNKOWN".to_string(),
        }

        if self.status == "ERROR" {
            self.message = response
                .status_message
                .as_ref()
                .map(String::as_str)
                .unwrap_or_default()
                .to_string();
            self.exit_code = 3;
            self.ready = true;
        } else if self.status == "IN_PROGRESS" || self.status == "READY" || self.status == "DNS" {
            self.exit_code = 0;
            self.ready = false;
        } else {
            self.message = response
                .status_message
                .as_ref()
                .map(String::as_str)
                .unwrap_or_default()
                .to_string();
            self.exit_code = 3;
            self.ready = true;
        }
    }

    fn set_ready(&mut self, status_message: &str) {
        self.status = status_message.to_string();
        if self.status == "Ready" {
            self.ready = true;
        }
    }

    fn set_exit_code(&mut self) {
        match self.grade.as_str() {
            "A" | "A+" => self.exit_code = 0,
            "A-" => self.exit_code = 1,
            "B" | "C" | "D" | "E" | "F" | "M" | "T" => self.exit_code = 2,
            _ => self.exit_code = 2,
        }
    }
}

struct Params {
    domain: DomainName,
    caching: String,
    publish: String,
}

impl Params {
    fn new() -> Params {
        Params {
            domain: "www.example.com".parse().unwrap(),
            caching: "&fromCache=off".to_string(),
            publish: "&publish=off".to_string(),
        }
    }

    fn caching(&mut self, switch: bool) {
        self.caching = match switch {
            true => "&fromCache=on".to_string(),
            false => "&fromCache=off".to_string(),
        }
    }

    fn publish(&mut self, switch: bool) {
        self.publish = match switch {
            true => "&publish=on".to_string(),
            false => "&publish=off".to_string(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        println!("CLI parameters: {:?}", &cli);
    }

    let mut status = Status::new();

    let mut count = 0;
    let bar = ProgressBar::new(cli.attemps.into());
    while status.ready == false {
        count += 1;

        let api_response_body = get_api_body(&cli);
        status = process_response_body(api_response_body, status);

        if status.ready == false {
            let pause_duration = Duration::from_secs(10);
            thread::sleep(pause_duration);
        }
        if count > cli.attemps {
            break;
        }

        if cli.progress {
            bar.inc(1);
        }
    }
    if cli.progress {
        bar.finish();
    }

    print_result(&status, &cli);
    exit(status.exit_code);
}

fn get_api_body(cli: &Cli) -> String {
    let mut params = Params::new();
    params.caching(cli.from_cache);
    params.publish(cli.publish);
    params.domain = match cli.domain.parse() {
        Ok(domain) => domain,
        Err(error) => {
            panic!("Invalid domain URL: {}", error);
        }
    };

    let request_url = format!(
        "{}{}{}{}",
        "https://api.ssllabs.com/api/v3/analyze?host=", params.domain, params.publish, params.caching
    );
    let response = get(request_url);
    let content = match response {
        Ok(content) => content.text().unwrap(),
        Err(error) => {
            panic!("API error: {}", error);
        }
    };
    if cli.verbose {
        println!("API Response: {}", content);
    }
    content
}

fn process_response_body(body: String, mut status: Status) -> Status {
    let response: Response = serde_json::from_str(&body).unwrap();
    // i.e. Unable to resolve domain name
    status.set_response(&response);
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
                status.set_ready(status_message);
                status.grade = grade.to_string();
                status.set_exit_code();
            } else {
                status.exit_code = 3;
                status.status = "ERROR".to_string();
                status.error = "Endpoint not ready".to_string();
            }
        }
        None => {
            status.exit_code = 3;
            status.status = "ERROR".to_string();
            status.error = "No endpoint".to_string();
        }
    }
    status
}

fn print_result(status: &Status, cli: &Cli) {
    if status.grade.is_empty() && !status.message.is_empty() {
        println!("{}: {}", status.status, status.message);
    } else if !status.grade.is_empty() && status.message.is_empty() {
        println!("{}", status.grade);
    } else if status.exit_code != 0 {
        println!("{}: {}", status.status, status.error);
    } else {
        println!("{} {} {}", status.status, status.message, status.error)
    }

    if cli.verbose {
        println!("{:?}", status);
    };
}
