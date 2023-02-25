use addr::DomainName;
use clap::Parser;
use indicatif::ProgressBar;
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::process::exit;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use strum_macros::{Display, EnumString};

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
    status: State,
    error: Option<String>,
    grade: Option<Grade>,
    message: Option<String>,
    exit_code: i32,
}

#[derive(Display, Debug, Eq, PartialEq, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
enum State {
    Dns,
    Error,
    #[strum(
        serialize = "IN_PROGRESS",
        serialize = "INPROGRESS",
        serialize = "IN PROGRESS"
    )]
    InProgress,
    Ready,
    Unknown,
}

#[derive(Display, Debug, PartialEq, EnumString)]
enum Grade {
    #[strum(serialize = "A+")]
    APlus,
    A,
    #[strum(serialize = "A-")]
    AMinus,
    B,
    C,
    D,
    E,
    F,
    M,
    T,
}

impl Status {
    fn set_response(&mut self, response: &Response) {
        if !response.status.is_empty() {
            self.status = match State::from_str(response.status.as_str()) {
                Ok(st) => st,
                Err(e) => panic!("Error occurred: {:?}", e),
            };
        }

        if self.status == State::Error {
            self.message = response
                .status_message
                .as_deref()
                .map(|status| status.to_string());
            self.exit_code = 3;
            self.ready = true;
        } else if self.status == State::InProgress
            || self.status == State::Ready
            || self.status == State::Dns
        {
            self.exit_code = 0;
            self.ready = false;
        } else {
            self.message = response
                .status_message
                .as_deref()
                .map(|status| status.to_string());
            self.exit_code = 3;
            self.ready = true;
        }
    }

    fn set_ready(&mut self, status_message: &str) {
        if State::from_str(status_message).is_ok() {
            self.status = State::from_str(status_message).unwrap();
        };
        if self.status == State::Ready {
            self.ready = true;
        }
    }

    fn set_exit_code(&mut self) {
        if self.grade.is_some() {
            match self.grade {
                Some(Grade::A) | Some(Grade::APlus) => self.exit_code = 0,
                Some(Grade::AMinus) => self.exit_code = 1,
                Some(Grade::B) | Some(Grade::C) | Some(Grade::D) | Some(Grade::E)
                | Some(Grade::F) | Some(Grade::M) | Some(Grade::T) => self.exit_code = 2,
                _ => self.exit_code = 2,
            }
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Status {
            ready: false,
            status: State::Unknown,
            error: None,
            grade: None,
            message: None,
            exit_code: 0,
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

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    if cli.verbose {
        println!("CLI parameters: {:?}", &cli);
    }

    let mut status = Status::default();

    let mut count = 0;
    let bar = ProgressBar::new(cli.attemps.into());
    while !status.ready {
        count += 1;

        let api_response_body = match get_api_body(&cli) {
            Ok(st) => st,
            Err(e) => panic!("{}", e),
        };

        status = match process_response_body(api_response_body, status) {
            Ok(st) => st,
            Err(e) => panic!("{}", e),
        };

        if !status.ready {
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

fn get_api_body(cli: &Cli) -> Result<String, Box<dyn Error>> {
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
        "https://api.ssllabs.com/api/v3/analyze?host=",
        params.domain,
        params.publish,
        params.caching
    );
    let response = get(request_url);
    let content = match response {
        Ok(content) => content.text().unwrap(),
        Err(e) => return Err(Box::new(e)),
    };
    if cli.verbose {
        println!("API Response: {}", content);
    }
    Ok(content)
}

fn process_response_body(body: String, mut status: Status) -> Result<Status, Box<dyn Error>> {
    let response: Response = serde_json::from_str(&body).unwrap();
    // i.e. Unable to resolve domain name
    status.set_response(&response);
    // Continue otherwise
    match response.endpoints {
        Some(endpoints) => {
            if let Some(endpoint) = endpoints.first() {
                let grade = endpoint.grade.as_deref().unwrap_or_default();
                let status_message = endpoint.status_message.as_deref().unwrap_or_default();
                status.set_ready(status_message);
                if !grade.is_empty() {
                    status.grade = Some(Grade::from_str(grade).unwrap());
                }
                status.set_exit_code();
            } else {
                status.exit_code = 3;
                status.status = State::Error;
                status.error = Some("Endpoint not ready".to_string());
            }
        }
        None => {
            status.exit_code = 3;
            status.status = State::Error;
            status.error = Some("No endpoint".to_string());
        }
    }
    Ok(status)
}

fn print_result(status: &Status, cli: &Cli) {
    if status.grade.is_some() {
        println!("{}", status.grade.as_ref().unwrap());
    }

    if cli.verbose {
        println!("{:?}", status);
    };
}
