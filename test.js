const rl = require('readline').createInterface({
  input: require('fs').createReadStream('sites.txt'),
  crlfDelay: Infinity,
});
const visited = new Set();

rl.on('line', (line) => {
  line = line.trim();
  if (line.length == 0) return;
  if (visited.has(line)) {
    throw new Error(`${line} was visited already!`);
  } else {
    visited.add(line);
  }
});

rl.on('close', () => console.log('Success!'));
