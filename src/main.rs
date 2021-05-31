use lazy_static::lazy_static;
use regex::Regex;
use std::collections::{HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::fs;
use num_cpus;
use std::io::prelude::*;
use reqwest;

lazy_static! {
    static ref HREF_PATTERN: Regex = Regex::new(r#"href=['"]([^'"]+?)['"]"#).unwrap();
    // (?!.*(js|css|jpg|jpeg|png|mp4|mp3|svg))http[s]??:\/\/.+? => error: look-around, including look-ahead and look-behind, is not supported
    static ref LINK_PATTERN: Regex = Regex::new(r#"http[s]??://.+?"#).unwrap();
}

fn write_to_output_file(output_file_path: &str, link: String) -> Result<(), std::io::Error> {
    let output_file_opts = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&output_file_path);
    match output_file_opts {
        Ok(mut file) => {
            if let Err(e) = writeln!(file, "{}\n", link) {
                println!("Failed to write to output file for link {}: {}", link, e);
                return Err(e);
            }
            Ok(())
        }
        Err(e) => {
            println!(
                "Failed to open output file to persist initial links crawled: {}",
                e
            );
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output_file_path = String::from("./sites.txt");
    match std::env::var_os("OUTPUT_FILE_PATH") {
        Some(file_path) => {
            output_file_path = file_path.into_string().unwrap();
        }
        None => {
            println!("OUTPUT_FILE_PATH env var not provided. Using default");
        }
    }
    let mut seed_link: String = String::from("https://devurls.com");
    match std::env::var_os("SEED_LINK") {
        Some(link) => {
            seed_link = link.into_string().unwrap();
        }
        None => {
            println!("SEED_LINK env var not provided. Using default");
        }
    }

    let body = reqwest::get(&seed_link)
        .await?
        .text()
        .await?;
    let mut initial_links: Vec<String> = Vec::new();
    for link_capture in HREF_PATTERN.captures_iter(body.as_str()) {
        let link = &link_capture[1];
        if LINK_PATTERN.is_match(link) {
            initial_links.push(String::from(link));
        }
    }

    println!("initial_link: {:?}", initial_links);
    let il_len = initial_links.len();
    if il_len == 0 {
        panic!("No initial_links to crawl for seed link: {}", &seed_link);
    }

    let visited_links: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let crawl_number = Arc::new(Mutex::new(0));
    let num_cpus = num_cpus::get();
    let total_threads = if il_len < num_cpus { il_len } else { num_cpus * 2 };
    let mut threads = Vec::new();
    for i in 0..total_threads {
        let link = initial_links[i].clone();
        let output_path = output_file_path.clone();
        let visited = Arc::clone(&visited_links);
        let crawls = Arc::clone(&crawl_number);

        let thread = thread::spawn(move || {
            let thread_id = thread::current().id();
            println!("Spawning thread {:?} with link: {}", thread_id, link);
            let link_clone = link.clone();

            visited.lock().unwrap().insert(String::from(link));

            let mut links_to_crawl: Vec<String> = Vec::new();

            println!("links_to_crawl.len() pre: {}",links_to_crawl.len());
            links_to_crawl.push(String::from(link_clone));
            println!("links_to_crawl.len() post: {}",links_to_crawl.len());

            while !links_to_crawl.is_empty() {
                let current_link = links_to_crawl.remove(0);

                println!("current_link {}", current_link);

                match write_to_output_file(&output_path, current_link.clone()) {
                    Ok(_) => {},
                    Err(_e) => {},
                }

                println!("Thread {:?} is crawling {}", thread_id, current_link);

                let mut crawls_guard = crawls.lock().unwrap();

                *crawls_guard += 1;

                std::mem::drop(crawls_guard);
                
                let req_link = current_link.clone();

                println!("req_link");

                let mut visited_guard = visited.lock().unwrap();

                visited_guard.insert(String::from(current_link));

                std::mem::drop(visited_guard);

                println!("reqwest::blocking::get(req_link)");

                match reqwest::blocking::get(req_link) {
                    Ok(res) => {
                        println!("res {:?}", res);
                        match res.text() {
                            Ok(res_text) => {
                                println!("res_text: {:?}", res_text);
                                for link_capture in HREF_PATTERN.captures_iter(res_text.as_str()) {
                                    let new_link = &link_capture[1];
                                    if LINK_PATTERN.is_match(new_link) {
                                        println!("new_link: {:?}", new_link);
                                        if !visited.lock().unwrap().contains(new_link) {
                                            links_to_crawl.push(String::from(new_link));
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                println!("res.text() error: {:?}", e);
                            }
                        }
                    },
                    Err(e) => {
                        println!("reqwest::blocking::get(req_link) error: {:?}", e);
                    }
                } 
            }
        });

        threads.push(thread);
    }


    Ok(())
}

