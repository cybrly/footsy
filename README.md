# footsy

**footsy** is a simple command-line utility that scans a network for web servers on common ports and displays their HTTP status codes and page titles. It also provides color-coded output for easy identification of responses.

## Features

- Scan local/internal IPs with customizable subnet ranges (e.g., `/24`, `/16`, `/8`).
- Identify active web servers running on common ports.
- Display HTTP status codes and page titles.
- Color-coded output to easily distinguish between success (2xx), redirection (3xx), client error (4xx), and server error (5xx) responses.
- Progress indicator to track scan progress.

## Supported Ports

- 80 (HTTP)
- 443 (HTTPS)
- 8008
- 3000
- 5000
- 9080
- 9443
- 8000
- 8001
- 8080
- 8443
- 9000
- 9001

## Usage

```bash
footsy <subnet_size>
```

### Arguments

- **subnet_size**: Size of the subnet to scan, e.g., 24 for `/24`, 16 for `/16`. The default value is `24`.

### Example

```bash
footsy 24
```

This will scan the local network with a subnet size of `/24`.

## Installation

To install `footsy`, you need to have Rust and Cargo installed. Then, you can install the application via Cargo:

```bash
cargo install footsy
```

Alternatively, clone the repository and build it manually:

```bash
git clone https://github.com/cybrly/footsy.git
cd footsy
cargo build --release
```

## Contributing

Feel free to fork the repository and submit pull requests. All contributions are welcome!

## License

This project is licensed under the MIT License. See the LICENSE file for more details.
