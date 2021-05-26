use async_recursion::async_recursion;
use hyper::{Body, Client, Error, Uri};
use hyper_tls::HttpsConnector;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::os;
use std::str;
use std::thread;
use std::io::prelude::*;
use num_cpus;

lazy_static! {
    static ref HREF_PATTERN: Regex = Regex::new(r#"href=['"]([^'"]+?)['"]"#).unwrap();
    static ref LINK_PATTERN: Regex = Regex::new(r#"http[s]??://.+?[^js|css|jpg|jpeg|png|mp4|mp3]+?"#).unwrap();
}

fn print_fetch_error(site: String, error: Error) {
    println!("****ERROR fetching from site ({}): {:?}", site, error);
}

#[async_recursion(?Send)]
async fn fetch_links_from<Cb: FnOnce(Option<Vec<String>>)>(site: String, callback: Cb)  {
    let mut links: Vec<String> = Vec::new();
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);
    let site_uri: Uri = site.parse().unwrap();
    match client.get(site_uri).await {
        Ok(res) => {
            match hyper::body::to_bytes(res.into_body()).await {
                Ok(body_bytes) => {
                    match String::from_utf8(body_bytes.to_vec()) {
                        Ok(html) => {
                            for link_capture in HREF_PATTERN.captures_iter(html.as_str()) {
                                let link = &link_capture[1];
                                if LINK_PATTERN.is_match(link) {
                                    println!("link: {:?}", link);
                                    links.push(String::from(link));
                                }
                            }
                            callback(Some(links))
                        }, 
                        Err(e) => {
                            println!("Invalid UTF-8 sequence: {}", e)
                        },
                    };
                },
                Err(e) => {
                      print_fetch_error(site, e);
                },
            };
        }
        Err(e) => {
            let message: String = e.to_string().clone();
            let site_clone = site.clone();

            print_fetch_error(site, e);

            if message.contains("'http:' not supported:") {
                fetch_links_from(site_clone, callback).await;
            } else {
                callback(None);
            }
        }
    }
}

#[tokio::main] // As a macro attribute, it's signature is (_attr: TokenStream, item: TokenStream) -> TokenStream so here the item = our function. It can process the token stream of the token including the "async keyword"
async fn main() {
    let mut output_file_path = String::from("./sites.txt");
    match std::env::var_os("OUTPUT_FILE_PATH") {
        Some(file_path) => {
            output_file_path = file_path.into_string().unwrap();
        }
        None => {
            println!("OUTPUT_FILE_PATH env var not provided. Using default");
        }
    }
    let mut seed_link: String = String::from("https://medium.com/tag/web-scraping");
    match std::env::var_os("SEED_LINK") {
        Some(link) => {
            seed_link = link.into_string().unwrap();
        }
        None => {
            println!("SEED_LINK env var not provided. Using default");
        }
    }

    let mut visited_link_set: HashSet<String> = HashSet::new();
    let mut crawl_number: i32 = 0;

    fetch_links_from(seed_link, |result: Option<Vec<String>>| {
        match result {
            Some(links) => {
                let mut total_log_cpus = num_cpus::get();
                let links_len = links.len();
                if links_len < total_log_cpus {
                    total_log_cpus = links_len;
                }

                println!("total_log_cpus: {}", total_log_cpus);

                let first_links = &links[0..total_log_cpus];
                if total_log_cpus < links_len {
                    for link in &links[total_log_cpus+1..] {
                        println!("link: {}", link);
                        if !visited_link_set.contains(link) {
                            crawl_number += 1;
                            visited_link_set.insert(link.clone());
                            let output_file_opts = fs::OpenOptions::new()
                                .write(true)
                                .append(true)
                                .open(&output_file_path);
                            match output_file_opts {
                                Ok(mut file) => {
                                    // let link_str: &str = link;
                                    if let Err(e) = writeln!(file, "{}", link) {
                                        panic!(format!("Failed to write to output file for link: {}", link));
                                    }
                    
                                }, 
                                Err(e) => {
                                    panic!("Failed to open output file to persist initial links crawled.");
                                }
                            }
                        }

                    }
                }
            },
            None => {
                panic!("Failed to fetch initial links for crawling. Please try another link.")
            }
        }
    }).await;
}
