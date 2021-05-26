# Developed using node v14.9.0 (npm v6.14.11)

### To run the crawler

```
node crawl.js
```

### Set seed link

```
SEED_LINK=https://www.github.com node crawl.js
```

### Set output file path (default: sites.txt) [**Make sure to create the output file first**]

```
SEED_LINK=https://www.youtube.com OUTPUT_FILE_PATH=./output.txt node crawl.js
```

### Persist logs

```
SEED_LINK=https://www.youtube.com OUTPUT_FILE_PATH=./output.txt node crawl.js > logs.txt
```

### Delete output and/or log file(s)

```
make clean
```
