// import {request} from 'http';
const url = require('url');
const spawn = require('child_process').spawn;
const request = require('http').request;
const net = require('net');

function hc(uri) {
  process.stdout.write('.');
  // console.log(arguments);
  return new Promise((resolve, reject) => {
    const parsed = url.parse(uri);
    // console.log(parsed);
    const req = request({
      method: 'get',
      hostname: parsed.hostname,
      port: parsed.port || 80,
      path: '/',
    });
    req.on('response', function(message) {
      // console.log('response');
      // console.log(message);
      message.setEncoding('utf8');
      message.on('error', function(err) {
        console.error(err);
        reject(err);
      });
      message.on('data', function() {});
      message.on('end', function() {
        // console.log('hc');
        resolve();
      });
    });
    req.on('error', function(err) {
      console.error(err);
      reject(err);
    });
    req.end();
    // , (err, response, body) => {
    //   console.log(err, response, body);
    //   if (err) {
    //     console.error(err);
    //     reject(err);
    //   }
    //   else {
    //     console.log('hc');
    //     resolve();
    //   }
    // });
  });
}

function loop(cmd, cmdOpts, uri, tcpPort) {
  // console.log(arguments);
  return hc(uri)
  // return Promise.resolve()
  .then(() => {
    return new Promise((resolve, reject) => {
      const child = spawn(cmd[0], cmd.slice(1), cmdOpts);
      const parsed = url.parse(uri);
      // console.log('connecting');
      const req = request({
        method: 'POST',
        hostname: parsed.hostname,
        port: parsed.port || 80,
        path: parsed.pathname,
        headers: {
          'Transfer-Encoding': 'chunked',
        },
      });
      // const req = net.createConnection({port: tcpPort, host: parsed.hostname});
      req.setNoDelay(true);
      // child.stdout.pipe(req);
      let bufferIndex = 0;
      let buffer = new Buffer(96000);
      const chunk = new Buffer(192);
      // child.stdout.on('data', function(data) {
      //   if (!buffer) {return;}
      //   // console.log(`send ${data.length} bytes`);
      //   // req.write(chunk);
      //   for (let i = 0; i < data.length; i++) {
      //     buffer[bufferIndex++] = data[i];
      //   }
      //   // let i = 0;
      //   // for (; i < (bufferIndex / chunk.length | 0); i++) {
      //   //   for (let j = 0; j < chunk.length; j++) {
      //   //     chunk[j] = buffer[i * chunk.length + j];
      //   //   }
      //   //   req.write(chunk);
      //   // }
      //   // for (let j = i * chunk.length; j < bufferIndex; j++) {
      //   //   buffer[j - i * chunk.length] = buffer[j];
      //   // }
      //   // console.log(`sent ${i} chunks. ${bufferIndex - i * chunk.length} left over`);
      //   // bufferIndex -= i * chunk.length;
      // });
      child.stdout.pause();
      function writeLoop() {
        try {
        let data = child.stdout.read(192);
        if (data) {
          // process.stdout.write(data.length.toString(16) + ' ');
          for (let i = 0; i < data.length; i++) {
            buffer[bufferIndex++] = data[i];
          }
        }
        }
        catch (e) {console.error(e);}
        if (buffer && bufferIndex > chunk.length) {
          for (let j = 0; j < chunk.length; j++) {
            chunk[j] = buffer[j];
          }
          for (let j = 0; j < bufferIndex - chunk.length; j++) {
            buffer[j] = buffer[j + chunk.length];
          }
          bufferIndex -= chunk.length;
          req.write(chunk, writeLoop);
          // console.log(Date.now());
        }
        else if (buffer) {
          setTimeout(writeLoop, 0);
        }
      }
      writeLoop();
      console.log('start write loop');
      // child.stdout.on('end', function() {
      //   req.end();
      // });
      // child.stderr.pipe(process.stderr);
      req.on('response', function(message) {
        message.on('data', function() {});
        message.on('end', function() {
          buffer = null;
          child.kill();
          reject();
        });
        message.on('error', function(err) {
          console.error(err);
          buffer = null;
          child.kill();
          reject();
        });
      });
      req.on('error', function(err) {
        console.error(err);
        buffer = null;
        child.kill();
        reject(err);
      });
      child.on('exit', function() {
        buffer = null;
        req.end();
        reject();
      });
    });
  })
  .then(() => loop(cmd, cmdOpts, uri), () => {
    return new Promise(resolve => setTimeout(resolve, 2000))
    .then(() => loop(cmd, cmdOpts, uri));
  });
}

if (!process.argv[2] || process.argv[2] === 'all' || process.argv[2] === 'spotify') {
loop(
  // ['sox', '|sox -d -p', '-r', '48000', '-b', '16', '-c', '2', '-t', 'raw', '-'],
  ['sox', '-d', '-b', '16', '-r', '48000', '-c', '2', '-t', 'raw', '-'],
  // ['sox', '-d', '-t', 'raw', '-'],
  {
    stdio: ['ignore', 'pipe', 'ignore'],
    env: Object.assign({}, process.env, {
      // AUDIODEV: 'Spotify',
      // AUDIODEV: 'Chrome',
    }),
  },
  // 'http://127.0.0.1:7777/music'
  // 'http://Tessel-02A32DCC461B.local/music'
  'http://cello.local/music'
  // 'http://10.31.0.125/music'
  // 'http://10.0.1.20/music'
  ,
  7777
);
}

if (process.argv[2] === 'all' || process.argv[2] === 'chrome') {
loop(
  ['sox', '|AUDIODEV=Chrome sox -d -p', '-r', '48000', '-b', '16', '-c', '2', '-t', 'raw', '-'],
  // ['sox', '-d', '-b', '16', '-r', '48000', '-c', '2', '-t', 'raw', '-'],
  // ['sox', '-d', '-t', 'raw', '-'],
  {
    stdio: ['ignore', 'pipe', 'ignore'],
    env: Object.assign({}, process.env, {
      AUDIODEV: 'Chrome',
      // AUDIODEV: 'Hijack',
    }),
  },
  'http://Tessel-02A32DCC461B.local/chrome'
  // 'http://10.0.1.20/chrome'
  ,
  7778
);
}
