use async_channel::{Receiver, Sender};
use async_recursion::async_recursion;
use core::panic;
use hyper::{Body, Client, Error, Uri};
use hyper_tls::HttpsConnector;
use lazy_static::lazy_static;
use num_cpus;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::future::Future;
use std::io::prelude::*;
use std::process;
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::ThreadId;

// TODO: clean this shit up

lazy_static! {
    static ref HREF_PATTERN: Regex = Regex::new(r#"href=['"]([^'"]+?)['"]"#).unwrap();
    static ref LINK_PATTERN: Regex =
        Regex::new(r#"http[s]??://.+?[^js|css|jpg|jpeg|svg|png|mp4|mp3]+?"#).unwrap();
}
#[derive(Debug)]
struct CrawlUpdate {
    ok_to_crawl: bool,
    link: String,
    crawl_number: i32,
    worker_id: ThreadId,
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
            // let link_str: &str = link;
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
    Cb: FnOnce(Option<Vec<String>>) -> Fut,
    Fut: Future,
{
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
                        }
                        Err(e) => {
                            println!("Invalid UTF-8 sequence: {}", e);
                            callback(None)
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
                callback(None);
            }
        }
    }
}

async fn handle_worker_thread_crawl(
    sender_thread_clone: Sender<CrawlUpdate>,
    received_thread_clone: Arc<Mutex<Receiver<CrawlUpdate>>>,
    link_thread_clone: String,
    crawl_number: i32,
) {
    let worker_id = thread::current().id();

    println!(
        "worker {:?} fetching links to crawl from: {:?}",
        worker_id, link_thread_clone
    );

    // NOTE: left off here (issues from ownership and lifetime of the function below)
    fetch_links_from(
        link_thread_clone.clone(),
        |t_result: Option<Vec<String>>| async {
            match t_result {
                Some(t_res_links) => {
                    println!("t_res_links: {:?}", t_res_links);
                    if t_res_links.len() == 0 {
                        return;
                    }
                    for t_link in t_res_links {
                        let send_result = sender_thread_clone.send(CrawlUpdate {
                            ok_to_crawl: false,
                            crawl_number: crawl_number,
                            link: t_link.clone(),
                            worker_id: worker_id.clone(),
                        });
                        match send_result.await {
                            Ok(_) => {
                                println!(
                                    "worker {:?} sent CrawlUpdate through channel",
                                    worker_id.clone()
                                );
                            }
                            Err(e) => {
                                panic!("Send error from worker {:?}: {:?}", worker_id.clone(), e);
                            }
                        }
                    }
                }
                None => {
                    println!("worker {:?} received no links to crawl", worker_id.clone());
                    process::exit(0);
                }
            }
        },
    )
    .await;

    let wt_recv = received_thread_clone.lock().unwrap();
    match wt_recv.recv().await {
        Ok(message) => {
            println!(
                "worker {:?} received a crawl update message: {:?}",
                worker_id.clone(),
                message
            );
        }
        Err(e) => {
            println!("main thread RecvError: {:?}", e);
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

    fetch_links_from(seed_link, |result: Option<Vec<String>>| async {
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
                    for link in &links[total_log_cpus + 1..] {
                        if !visited_link_set.contains(link) {
                            crawl_number += 1;
                            visited_link_set.insert(link.clone());
                            write_to_output_file(&output_file_path, link.clone());
                        }
                    }
                }

                let total_threads = total_log_cpus;
                let (sender, receiver): (Sender<CrawlUpdate>, Receiver<CrawlUpdate>) =
                    async_channel::unbounded();
                let arc_receiver = Arc::new(Mutex::new(receiver));
                let mut worker_threads = Vec::new();

                for i in 0..total_threads {
                    let link = &first_links[i];

                    crawl_number += 1;

                    visited_link_set.insert(link.clone());

                    let sender_thread_clone = sender.clone();
                    let received_thread_clone = Arc::clone(&arc_receiver);
                    let link_thread_clone = link.clone();
                    // NOTE: Left off here
                    let worker_thread = thread::spawn(move || async {
                        handle_worker_thread_crawl(
                            sender_thread_clone,
                            received_thread_clone,
                            link_thread_clone,
                            crawl_number,
                        )
                        .await;
                    });

                    worker_threads.push(worker_thread);
                }

                let recv = arc_receiver.lock().unwrap();
                match recv.recv().await {
                    Ok(message) => {
                        println!("(PARENT) Received a crawl update message: {:?}", message);
                    }
                    Err(e) => {
                        println!("main thread RecvError: {:?}", e);
                    }
                }

                for thread in worker_threads {
                    let worker_id = thread.thread().id();
                    match thread.join() {
                        Ok(_) => {
                            println!("worker {:?} exited successfully", worker_id.clone(),);
                        }
                        Err(e) => {
                            println!("worker {:?} exited with error: {:?}", worker_id.clone(), e);
                        }
                    }
                }
            }
            None => {
                panic!("Failed to fetch initial links for crawling. Please try another link.")
            }
        }
    })
    .await;
}
