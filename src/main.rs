use async_recursion::async_recursion;
use core::panic;
use hyper::{Body, Client, Error, Uri};
use hyper_tls::HttpsConnector;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::future::Future;
use std::io::prelude::*;
use std::str;

// TODO: clean this shit up

lazy_static! {
    static ref HREF_PATTERN: Regex = Regex::new(r#"href=['"]([^'"]+?)['"]"#).unwrap();
    static ref LINK_PATTERN: Regex =
        Regex::new(r#"http[s]??://.+?[^js|css|jpg|jpeg|svg|png|mp4|mp3]+?"#).unwrap();
}

fn print_fetch_error(site: String, error: Error) {
    println!("****ERROR fetching from site ({}): {:?}", site, error);
}

fn write_to_output_file(output_file_path: &str, link: String) {
    let output_file_opts = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&output_file_path);
    match output_file_opts {
        Ok(mut file) => {
            if let Err(e) = writeln!(file, "{}", link) {
                panic!("Failed to write to output file for link {}: {}", link, e);
            }
        }
        Err(e) => {
            panic!(
                "Failed to open output file to persist initial links crawled: {}",
                e
            );
        }
    }
}

#[async_recursion(?Send)]
async fn fetch_links_from<Cb, Fut>(site: String, callback: Cb)
where
    Cb: FnOnce(Option<HashSet<String>>) -> Fut,
    Fut: Future,
{
    let mut links: HashSet<String> = HashSet::new();
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
                                    // println!("link: {:?}", link);
                                    links.insert(String::from(link));
                                }
                            }
                            callback(Some(links)).await
                        }
                        Err(e) => {
                            println!("Invalid UTF-8 sequence: {}", e);
                            callback(None).await
                        }
                    };
                }
                Err(e) => {
                    print_fetch_error(site, e);
                }
            };
        }
        Err(e) => {
            let message: String = e.to_string().clone();
            let site_clone = site.clone();

            print_fetch_error(site, e);

            if message.contains("'http:' not supported:") {
                fetch_links_from(site_clone, callback).await;
            } else {
                callback(None).await;
            }
        }
    }
}

#[tokio::main]
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
    fetch_links_from(seed_link, |result: Option<HashSet<String>>| async {
        match result {
            Some(mut links) => {
                println!("links: {:?}", links);
                while !links.is_empty() {
                    for link in links.clone().iter() {
                        &links.remove(link);
                        if visited_link_set.contains(link) {
                            return;
                        }

                        crawl_number += 1;

                        println!("Crawl #{:?}: {:?}", crawl_number, link.clone());

                        visited_link_set.insert(link.clone());

                        write_to_output_file(&output_file_path, link.clone());

                        fetch_links_from(
                            link.clone(),
                            |fetch_result: Option<HashSet<String>>| async {
                                match fetch_result {
                                    Some(more_links) => {
                                        &links.extend(more_links);
                                        futures::future::ok::<(), ()>(())
                                    }
                                    None => {
                                        println!("No links to crawl.");
                                        futures::future::ok::<(), ()>(())
                                    }
                                }
                            },
                        )
                        .await
                    }
                }
            }
            None => {
                panic!("Failed to fetch initial links for crawling. Please try another link.");
            }
        }
    })
    .await;
}
