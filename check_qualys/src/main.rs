use clap::{Arg, ArgMatches, Command};
use reqwest::blocking::get;
use scraper::{Html, Selector};
use std::fs;
use std::io;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_options = get_cli_parameters();
    let mut is_ready = false;

    while is_ready == false {
        let mut body = "".to_string();
        if let Some(file) = cli_options.get_one::<String>("file") {
            body = get_file_body(file);
        } else if let Some(url) = cli_options.get_one::<String>("url") {
            body = get_url_body(url);
        }

        is_ready = process_response_body(body);

        if is_ready == false {
            let pause_duration = Duration::from_secs(5);
            thread::sleep(pause_duration);
        }
    }

    return Ok(());
}

fn get_url_body(url: &str) -> String {
    let request_url = format!(
        "{}{}{}",
        "https://www.ssllabs.com/ssltest/analyze.html?d=".to_string(),
        url,
        "&hideResults=on&latest".to_string()
    );
    // println!("{}", request_url);
    let response = get(request_url).unwrap();

    // Capture the response body
    let body = response.text().unwrap();
    return body;
}

fn get_file_body(file: &str) -> String {
    match open_file(file) {
        Ok(file) => {
            return file;
        }
        Err(error) => {
            // Handle the error
            eprintln!("Failed to open file: {}", error);
        }
    }
    return "".to_string();
}

fn open_file(file_path: &str) -> Result<String, io::Error> {
    let file_result = fs::read_to_string(file_path)?;
    Ok(file_result)
}

fn get_cli_parameters() -> ArgMatches {
    let matches = Command::new("qualysapp")
        .arg(
            Arg::new("file")
                .help("Input file path")
                .short('f')
                .required(false),
        )
        .arg(
            Arg::new("url")
                .help("URL address")
                .short('u')
                .required(false),
        )
        .get_matches();

    return matches;
}

fn process_response_body(body: String) -> bool {
    let document = Html::parse_document(&body);
    let mut ready_status = false;

    // CDN case
    let cdn_table_selector = Selector::parse("table#multiTable tbody").unwrap();
    let cdn_row_selector = Selector::parse("tr").unwrap();
    // IP case
    let ip_selector = Selector::parse("div.reportTitle").unwrap();
    let ip_selector_rating = Selector::parse("div.rating_g").unwrap();

    // Find the table
    let html_result = match document.select(&cdn_table_selector).next() {
        Some(html) => (true, html),
        None => {
            if let Some(html_div) = document.select(&ip_selector).next() {
                // println!("{}", html_div.inner_html().as_str());
                (false, html_div)
            } else {
                panic!("Failed to find rating for a CDN or IP case");
            }
        }
    };

    if html_result.0 == true {
        // Find the first non-title row
        let first_row = html_result
            .1
            .select(&cdn_row_selector)
            .skip(2) // skip the title row
            .next()
            .unwrap();

        // Extract the ready status
        let cell_result_selector = Selector::parse("td").unwrap();
        let result_cell = first_row.select(&cell_result_selector);
        for cell in result_cell.clone() {
            if cell.text().collect::<String>().contains("Ready") {
                ready_status = true;
            }
        }

        // Extract the result value
        let result = result_cell.last();
        if let Some(cell) = result {
            println!("{}", cell.text().collect::<String>().trim());
        }
        return ready_status;
    } else {
        let ip_rating = match document.select(&ip_selector_rating).next() {
            Some(html) => html,
            None => {
                return false;
            }
        };
        println!("{}", ip_rating.inner_html().trim());
        ready_status = true;
        return ready_status;
    }
}
