- install nodejs
- npm install -g t2-cli
- install rustup
- `rustup toolchain install 1.12.0`
- `rustup default 1.12.0`
- `rustup target add mipsel-unknown-gnu`
- cargo tessel install sdk
- copy contents of usr tar over sdk folder (`~/.tessel/sdk/macos/toolchain-mipsel_24kec+dsp_gcc-4.8-linaro_uClibc-0.9.33.2`). Should result in a pkgconfig folder under `~/.tessel/sdk/macos/toolchain-mipsel_24kec+dsp_gcc-4.8-linaro_uClibc-0.9.33.2/lib`.
- configure env variables to compile with alsa (this could be skipped once sdk includes a pkgconfig binary)

  ```
  export PKG_CONFIG_PATH=${PKG_CONFIG_PATH}:~/.tessel/sdk/macos/toolchain-mipsel_24kec+dsp_gcc-4.8-linaro_uClibc-0.9.33.2/lib/pkgconfig
  export PKG_CONFIG_ALLOW_CROSS=1
  ```

- `t2 run tcp` (needs a sound card plugged into the top usb port)
- Use software like [Soundflower](https://rogueamoeba.com/freebies/soundflower/) or [Loopback](https://www.rogueamoeba.com/loopback/) to create a virtual audio device to capture sound on your system
- `node stream "Name of virtual sound device" tcp://name-of-tessel.local:7777`
