- nodejs
- npm install -g t2-cli
- install rustup
- `rustup toolchain add 1.12.0`
- `rustup default 1.12.0`
- `rustup target add mipsel-unknown-gnu`
- cargo tessel install sdk
- copy contents of usr tar over sdk folder (`~/.tessel/sdk/macos/toolchain-mipsel_24kec+dsp_gcc-4.8-linaro_uClibc-0.9.33.2`). Should result in a pkgconfig folder under `~/.tessel/sdk/macos/toolchain-mipsel_24kec+dsp_gcc-4.8-linaro_uClibc-0.9.33.2/lib`.
- configure env variables for alsa

  ```
  export PKG_CONFIG_PATH=${PKG_CONFIG_PATH}:~/.tessel/sdk/macos/toolchain-mipsel_24kec+dsp_gcc-4.8-linaro_uClibc-0.9.33.2/lib/pkgconfig
  export PKG_CONFIG_ALLOW_CROSS=1
  ```

- commands to find alsa names with
