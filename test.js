const rl = require('readline').createInterface({
  input: require('fs').createReadStream('sites.txt'),
  crlfDelay: Infinity,
});
const visited = {};

rl.on('line', (line) => {
  if (visited[line]) {
    throw new Error(`${line} was visited already!`);
  } else {
    visited[line] = true;
  }
});

rl.on('close', () => console.log('Success!'));
