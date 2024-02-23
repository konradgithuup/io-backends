# JULEA Object-Backend Extension

This project implements additional [JULEA](https://github.com/parcio/julea) Object-Backends.

It consists of a root package and four packages implementing JULEA backends for POSIX I/O, POSIX aio (using libaio), mmap, and io_uring.

Building the project will generate a dynamic library for each backend that can then be plugged into JULEA (Refer to the JULEA documentation on how to provide backends to JULEA).

## Setup

### Requirements

**Environment**

Some of the backends like io_uring are specific to the Linux kernel. Therefore, a Linux kernel of version 5.6 or higher is required.

**Dependencies**
- JULEA
  - libglib\-2.0\-dev
  - libbson\-dev
- [Rust]() (stable)

(Cargo takes care of the remaining dependencies)

### Installation

Check out the repository.

```bash
# in directory of your choice
git clone https://github.com/konradgithuup/io-backends.git
cd io-backends
```

Set an environment variable "JULEA_INCLUDE" to point bindgen to your JULEA installations header files.

```bash
export JULEA_PATH="path/to/JULEA/include"
```

For bindgen to be able to automatically generate JULEA bindings, the glib- and libbson headers must be provided like below.

```bash
export BINDGEN_EXTRA_CLANG_ARGS="$(pkg-config --cflags glib-2.0) $(pkg-config --cflags libbson-1.0)"
```

Set an environment variable "CONFIG_PATH" for the extension to find the backend runtime configurations. A set of valid configurations is already provided in `./sample_configs`.

```bash
export CONFIG_PATH="path/to/configs"
```

You can then validate the successful installation by executing the test suite. (The test suite will not necessarily fail if an error occurs. Instead it logs the error and carries on, if possible).

```bash
cargo test --lib
```