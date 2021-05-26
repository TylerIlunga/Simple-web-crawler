const http = require('http');
const https = require('https');
const cluster = require('cluster');
const fs = require('fs');
const { cpus } = require('os');

const HREF_PATTERN = /href=['"]([^'"]+?)['"]/g;
const LINK_PATTERN = /"http.+?[^js|png]+?"/g;

const visitedLinkMap = {};

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
      .on('error', (err) => {
        console.log(`****ERROR fetching from site (${site}): `, err.message);
        callback([]);
      });
  } catch (error) {
    console.log(`****ERROR fetching from site (${site}): `, error);
    if (error.message.includes('"http:" not supported')) {
      return fetchLinksFrom(site, true, callback);
    }
    callback([]);
  }
};

if (cluster.isMaster) {
  fetchLinksFrom(
    'https://medium.com/bipa-engineering/road-to-asynchronous-stability-f86afc3512a',
    false,
    (links) => {
      let numCPUs = cpus().length;
      const firstLinks = links.slice(0, numCPUs);
      console.log('firstLinks:', firstLinks);
      for (let i = 0; i < numCPUs; i++) {
        visitedLinkMap[firstLinks[i]] = true;

        const worker = cluster.fork({ startingLink: firstLinks[i] });

        console.log('spawning worker: ', worker.id);

        worker.on('message', (data) => {
          console.log('message from worker: ', data.id);
          if (visitedLinkMap[data.link]) {
            return worker.send({ okToCrawl: false, worker_id: worker.id });
          }

          visitedLinkMap[data.link] = true;

          fs.appendFile('./sites.txt', data.link + '\n', () => {});

          return worker.send({
            okToCrawl: true,
            link: data.link,
            worker_id: worker.id,
          });
        });
      }
    },
  );
} else {
  cluster.worker.on('message', (data) => {
    console.log('message from master: ', data);
    if (data.okToCrawl) {
      console.log('crawling: ', data.link);
      fetchLinksFrom(data.link, false, (links) => {
        if (links.length == 0) {
          return console.log(`worker #${cluster.worker.id} finished crawling.`);
        }
        links.forEach((link) => {
          cluster.worker.send({ link, id: cluster.worker.id });
        });
      });
    }
  });
  fetchLinksFrom(process.env.startingLink, false, (links) => {
    if (links.length > 0) {
      links.forEach((link) => {
        cluster.worker.send({ link, id: cluster.worker.id });
      });
    }
  });
}
