// import {request} from 'http';
const url = require('url');
const spawn = require('child_process').spawn;
const request = require('http').request;

function hc(uri) {
  console.log(arguments);
  return new Promise((resolve, reject) => {
    const parsed = url.parse(uri);
    console.log(parsed);
    const req = request({
      method: 'get',
      host: parsed.host,
      path: '/',
    });
    req.on('response', function(message) {
      console.log('response');
      // console.log(message);
      message.setEncoding('utf8');
      message.on('error', function(err) {
        console.error(err);
        reject(err);
      });
      message.on('data', function() {});
      message.on('end', function() {
        console.log('hc');
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

function loop(cmd, cmdOpts, uri) {
  // console.log(arguments);
  return hc(uri)
  .then(() => {
    return new Promise((resolve, reject) => {
      const child = spawn(cmd[0], cmd.slice(1), cmdOpts);
      const parsed = url.parse(uri);
      const req = request({
        method: 'POST',
        host: parsed.host,
        path: parsed.pathname,
      });
      child.stdout.pipe(req);
      // child.stderr.pipe(process.stderr);
      req.on('response', function(message) {
        message.on('data', function() {});
        message.on('end', function() {
          child.kill();
          reject();
        });
        message.on('error', function() {
          child.kill();
          reject();
        });
      });
      req.on('error', function(err) {
        child.kill();
        reject(err);
      });
      child.on('exit', function() {
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

loop(
  ['sox', '-d', '-b', '16', '-r', '48000', '-c', '2', '-t', 'raw', '-'],
  {
    stdio: ['ignore', 'pipe', process.stderr],
    env: Object.assign({}, process.env, {
      AUDIODEV: 'Spotify',
      // AUDIODEV: 'Hijack',
    }),
  },
  'http://Tessel-02A32DCC461B.local/music/'
);
