# :camera: `photosort` - A pictures/files organizer.

A simple CLI/daemon program to sorts your pictures/files.

## Getting started

Let's start by installing `photosort`.

### Installation

You can install `photosort` using one of the following methods:

```shell
# Using cargo
cargo install photosort

# Using docker
docker pull docker.io/negrel/photosort
# or podman
podman pull docker.io/negrel/photosort
```

### Usage

```shell
# Print help
photosort --help
```

Sort given directories/files:

```shell
photosort sort -r hardlink -r softlink "/path/to/dst/:file.name:" /path/to/src1 /path/to/src2 ...
```

Watch directories and sort them as new files are added:
```shell
photosort daemon -r hardlink "/path/to/dst/:file.name:" /path/to/src1 /path/to/src2
```

## Contributing

If you want to contribute to `photosort` to add a feature or improve the code contact
me at [negrel.dev@protonmail.com](mailto:negrel.dev@protonmail.com), open an
[issue](https://github.com/negrel/photosort/issues) or make a
[pull request](https://github.com/negrel/photosort/pulls).

## :stars: Show your support

Please give a :star: if this project helped you!

[![buy me a coffee](.github/bmc-button.png)](https://www.buymeacoffee.com/negrel)

## :scroll: License

MIT © [Alexandre Negrel](https://www.negrel.dev/)
