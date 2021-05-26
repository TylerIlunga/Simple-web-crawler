const cluster = require('cluster');
const fs = require('fs');
const http = require('http');
const https = require('https');
const os = require('os');

const HREF_PATTERN = /href=['"]([^'"]+?)['"]/g;
const LINK_PATTERN = /"http.+?[^js|css|jpg|jpeg|png|mp4|mp3]+?"/g;
const OUTPUT_FILE_PATH = process.env.OUTPUT_FILE_PATH || './sites.txt';
const SEED_LINK =
  process.env.SEED_LINK || 'https://medium.com/tag/web-scraping';

const visitedLinkSet = new Set();
let crawlNumber = 0;

const log = (overwrite, ...logArgs) => {
  if (overwrite) {
    return console.log(...logArgs);
  }
  if (process.env.DISABLE_LOGGING === 'true') {
    return;
  }
  return console.log(...logArgs);
};

const fetchLinksFrom = (site, useHTTP, callback) => {
  const networkProtocol = useHTTP ? http : https;
  try {
    networkProtocol
      .get(site, (res) => {
        let data = [];

        res.on('data', (chunk) => {
          data.push(chunk);
        });

        res.on('end', () => {
          const html = Buffer.concat(data).toString();
          let links = html.match(HREF_PATTERN);
          if (links == null || links == undefined) {
            return callback([]);
          }

          links = links
            .filter((link) => link.match(LINK_PATTERN))
            .map((link) => link.substring(5).replace(/\"/g, ''));

          callback(links);
        });
      })
      .on('error', (error) => {
        log(false, `****ERROR fetching from site (${site}): `, error.message);
        callback([]);
      });
  } catch (error) {
    log(false, `****ERROR fetching from site (${site}): `, error.message);
    if (error.message.includes('"http:" not supported')) {
      return fetchLinksFrom(site, true, callback);
    }
    callback([]);
  }
};

if (cluster.isMaster) {
  log(true, 'Crawling...');

  fetchLinksFrom(SEED_LINK, false, (links) => {
    let numCPUs = os.cpus().length;
    if (links.length < numCPUs) {
      numCPUs = links.length;
    }

    const firstLinks = links.slice(0, numCPUs);
    if (numCPUs < links.length) {
      links.slice(numCPUs + 1).forEach((link) => {
        if (!visitedLinkSet.has(link)) {
          crawlNumber++;
          visitedLinkSet.add(link);
          // NOTE: Since this a trival web crawler for the purpose of learning, the additional initial links are not crawled.
          fs.appendFile(OUTPUT_FILE_PATH, link + '\n', () => {});
        }
      });
    }

    for (let i = 0; i < numCPUs; i++) {
      crawlNumber++;

      visitedLinkSet.add(firstLinks[i]);

      const worker = cluster.fork({ startingLink: firstLinks[i] });
      log(false, 'spawning worker: ', worker.id);

      worker.on('message', (data) => {
        log(false, 'message from worker: ', data.id, data);
        if (visitedLinkSet.has(data.link)) {
          return worker.send({ okToCrawl: false, worker_id: worker.id });
        }

        crawlNumber++;

        visitedLinkSet.add(data.link);

        fs.appendFile(OUTPUT_FILE_PATH, data.link + '\n', () => {});

        return worker.send({
          crawlNumber,
          okToCrawl: true,
          link: data.link,
          worker_id: worker.id,
        });
      });

      worker.on('exit', (code, signal) => {
        if (signal) {
          return log(
            true,
            `worker #${worker.id} was killed by signal: ${signal}`,
          );
        }
        if (code !== 0) {
          return log(
            true,
            `worker #${worker.id} exited with error code: ${code}`,
          );
        }
        log(true, `worker #${worker.id} successfully exited`);
      });
    }
  });
} else {
  cluster.worker.on('message', (data) => {
    if (data.okToCrawl) {
      log(true, `Crawl #${data.crawlNumber}: ${data.link}`);
      fetchLinksFrom(data.link, false, (links) => {
        if (links.length == 0) {
          return;
        }
        links.forEach((link) => {
          cluster.worker.send({ link, id: cluster.worker.id });
        });
      });
    }
  });
  fetchLinksFrom(process.env.startingLink, false, (links) => {
    if (links.length === 0) {
      return process.exit(0);
    }
    links.forEach((link) => {
      cluster.worker.send({ link, id: cluster.worker.id });
    });
  });
}
