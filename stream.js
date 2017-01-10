const stream = require('stream');
const url = require('url');
const spawn = require('child_process').spawn;
const exec = require('child_process').exec;
const request = require('http').request;
const net = require('net');

function healthCheck(uri) {
  return new Promise((resolve, reject) => {
    const parsed = url.parse(uri);

    let req;
    if (parsed.protocol === 'http:') {
      req = request({
        method: 'get',
        hostname: parsed.hostname,
        port: 80,
        path: '/',
      });
      req.on('response', function(message) {
        message.setEncoding('utf8');
        message.on('error', reject);
        message.on('data', function() {});
        message.on('end', function() {resolve();});
      });
    }
    else {
      req = net.createConnection({port: parsed.port, host: parsed.hostname});
      req.on('data', function() {});
      req.on('end', function() {resolve();});
    }

    req.on('error', reject);
    req.end();
  });
}

function lowBufferChild(cmd, cmdOpts) {
  const child = spawn(cmd[0], cmd.slice(1), cmdOpts);

  let bufferIndex = 0;
  buffer = new Buffer(96000);
  const chunk = new Buffer(192);

  const originalStdout = child.stdout;
  originalStdout.pause();

  let eof = false;
  child.stdout = new stream.Readable();
  child.stdout._read = function() {
    if (eof) {
      this.push(null);
      return;
    }
  };

  function writeLoop() {
    try {
      if (eof) {
        child.stdout.push(null);
        return;
      }
      let data;
      while ((data = originalStdout.read(192)) && child.stdout.push(data));
    }
    catch (e) {console.error(e);}
    setTimeout(writeLoop, 0);
  };
  writeLoop();

  child.on('exit', function() {eof = true;});
  child.on('error', function() {eof = true;});
  originalStdout.on('end', function() {eof = true;});
  originalStdout.on('error', function() {eof = true;});

  return child;
}

function streamToServer(options, childStdout) {
  let buffer;
  return new Promise((resolve, reject) => {
    const parsed = url.parse(options.url);

    let req;
    if (parsed.protocol === 'http:') {
      console.log('connecting over http');
      req = request({
        method: 'POST',
        hostname: parsed.hostname,
        port: parsed.port || 80,
        path: parsed.pathname,
        headers: {
          'Transfer-Encoding': 'chunked',
        },
      });
    }
    else {
      console.log('connecting over tcp');
      req = net.createConnection({port: parsed.port, host: parsed.hostname});
    }

    req.setNoDelay(true);

    let bufferIndex = 0;
    buffer = new Buffer(96000);
    const chunk = new Buffer(192);

    let connected = false;
    req.on('response', function(message) {
      console.log('connected');
      message.on('data', function() {if (!connected) {connected = true; console.log('connected');}});
      message.on('end', function() {reject();});
      message.on('error', reject);
    });
    req.on('data', function() {if (!connected) {connected = true; console.log('connected');}});
    req.on('end', function() {reject();});
    req.on('error', reject);
    req.on('close', reject);

    childStdout.pipe(req);
  });
}

function loop(cmd, cmdOpts, uri, tcpPort) {
  return healthCheck(uri)
  .then(() => {
    console.log('found server');
    const child = lowBufferChild(cmd, cmdOpts);
    const closeHandle = (err) => {
      if (err) {console.error(err);}
      console.log('disconnected');
      try {
        child.kill();
      } catch (e) {}
    };
    return streamToServer({url: uri}, child.stdout)
    .then(closeHandle, closeHandle);
  })
  .then(() => loop(cmd, cmdOpts, uri), err => {
    console.log('attempting after error', err);
    return new Promise(resolve => setTimeout(resolve, 2000))
    .then(() => loop(cmd, cmdOpts, uri));
  });
}

var commands = {
  sox: ['sox', '-d', '-b', '16', '-r', '48000', '-c', '2', '-t', 'raw', '-'],
};

if (process.argv.length < 3) {
  console.log(`
    usage: node stream <AUDIO_DEVICE> <SERVER_ENDPOINT>

      AUDIO_DEVICE is any recordable input device on the system.

      SERVER_ENDPOINT is either a http or tcp url like:
        - http://mdns-server.local/post_endpoint
        - tcp://mdns-server.local:port_number
  `);
  return;
}

function checkSox() {
  return new Promise((resolve, reject) => {
    exec('sox --version', function(err) {
      if (err) {return reject(err);}
      else {resolve();}
    });
  });
}

checkSox()
.then(function() {
  loop(
    commands.sox,
    {
      stdio: ['ignore', 'pipe', 'ignore'],
      env: Object.assign({}, process.env, {
        AUDIODEV: process.argv[2],
      }),
    },
    process.argv[3]
  );
}, function() {
  console.error("Can't find sox. Please install it with 'apt' or 'brew'.");
});
