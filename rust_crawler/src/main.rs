use async_recursion::async_recursion;
use hyper::{Body, Client, Error, Uri};
use hyper_tls::HttpsConnector;
use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::os;
use std::str;
use std::thread;

fn print_fetch_error(site: String, error: Error) {
    println!("****ERROR fetching from site ({}): {:?}", site, error);
}

#[async_recursion(?Send)]
async fn fetch_links_from<Cb: FnOnce(Vec<String>)>(site: String, callback: Cb)  {
    let mut links: Vec<String> = Vec::new();
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);
    let site_uri: Uri = site.parse().unwrap();

    match client.get(site_uri).await {
        Ok(res) => {
            println!("Response: {}", res.status());
            println!("Headers: {:#?}\n", res.headers());

            // let body = res.body();

            match hyper::body::to_bytes(res.into_body()).await {
                Ok(body_bytes) => {
                    println!("body_bytes: {:?}", body_bytes);
                    match String::from_utf8(body_bytes.to_vec()) {
                        Ok(html) => {
                            println!("html: {}", html);
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
                callback(links);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let href_pattern: Regex = Regex::new(r"href=[']([^']+?)[']").unwrap();
    let link_pattern: Regex = Regex::new(r"http.+?[^js|css|jpg|jpeg|png|mp4|mp3]+?").unwrap();
    let mut output_file_path = String::from("./sites.txt");
    match std::env::var_os("OUTPUT_FILE_PATH") {
        Some(file_path) => {
            output_file_path = file_path.into_string().unwrap();
        }
        None => {
            // println!("OUTPUT_FILE_PATH env var not provided");
        }
    }
    let mut seed_link: String = String::from("https://medium.com/tag/web-scraping");
    match std::env::var_os("SEED_LINK") {
        Some(link) => {
            seed_link = link.into_string().unwrap();
        }
        None => {
            // println!("SEED_LINK env var not provided");
        }
    }

    let mut visited_link_set: HashSet<String> = HashSet::new();
    let mut crawlNumber = 0;

    fetch_links_from(seed_link, |links: Vec<String>| {
        println!("links: {:?}", links);
    }).await;

    Ok(())
}
