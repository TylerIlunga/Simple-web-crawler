# Developed using node v14.9.0 (npm v6.14.11) and rustc 1.52.1

### To run the crawler

- Node.js

```
node crawl.js
```

- Rust

```
cargo run
```

### Set seed link

- Node.js

```
SEED_LINK=https://medium.com/tag/web-scraping node crawl.js
```

- Rust

```
SEED_LINK=https://medium.com/tag/web-scraping cargo run
```

### Set output file path (default: sites.txt) [**Make sure to create the output file first**]

- Node.js

```
touch output.txt
SEED_LINK=https://devurls.com OUTPUT_FILE_PATH=./output.txt node crawl.js
```

- Rust

```
touch output.txt
SEED_LINK=https://devurls.com OUTPUT_FILE_PATH=./output.txt cargo run
```

### Persist logs

- Node.js

```
touch output.txt
SEED_LINK=https://www.youtube.com OUTPUT_FILE_PATH=./output.txt node crawl.js > logs.txt
```

- Rust

```
touch output.txt
SEED_LINK=https://www.youtube.com OUTPUT_FILE_PATH=./output.txt cargo run > logs.txt
```

### Delete output and/or log file(s)

```
make clean
```
