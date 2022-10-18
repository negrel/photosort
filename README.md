# :camera: `photosort` - A pictures/files organizer.

![push workflow](https://github.com/negrel/photosort/actions/workflows/push.yaml/badge.svg)

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
# Print help informations
photosort --help

# Using docker
docker run --rm -it negrel/photosort --help
```

Sort given `/path/to/src1` and `/path/to/src2` directories/files recursively using
`/path/to/dst/:file.name:` template. Files and directories are replicated using
hardlink and fallback to softlink if the primer fails.

```shell
photosort sort -r hardlink -r softlink "/path/to/dst/:file.name:" /path/to/src1 /path/to/src2 ...
```

Watch directories and sort them as new files are added:
```shell
photosort watch --daemon -r hardlink -r copy "/path/to/dst/:file.name:" /path/to/src1 /path/to/src2 ...
```

## Template variables

The following template variables are available for now. If you're missing other variables,
don't hesitate to make a PR !

| Variable | Description |
| :------- | :---------- |
| `file.path` | Absolute path to file. |
| `file.name` | File name. |
| `file.stem` | Extracts the stem (non-extension) portion of the filename. |
| `file.extension` | Extracts the extension part of the filename. |
| `file.md.creation_date` | File creation date in RFC3339 format. |
| `file.md.creation_date.year` | Year extracted from file creation date. |
| `file.md.creation_date.month` | Month extracted from file creation date. |
| `file.md.creation_date.day` | Day extracted from file creation date. |
| `exif.date` | Exif date in RFC3339 format. |
| `exif.date.year` | Year extracted from DateTime exif field. |
| `exif.date.month` | Month extracted from DateTime exif field. |
| `exif.date.day` | Day extracted from DateTime exif field. |

## Contributing

If you want to contribute to `photosort` to add a feature or improve the code contact
me at [negrel.dev@protonmail.com](mailto:negrel.dev@protonmail.com), open an
[issue](https://github.com/negrel/photosort/issues) or make a
[pull request](https://github.com/negrel/photosort/pulls).

## :stars: Show your support

Please give a :star: if this project helped you!

[![buy me a coffee](.github/images/bmc-button.png)](https://www.buymeacoffee.com/negrel)

## :scroll: License

MIT © [Alexandre Negrel](https://www.negrel.dev/)
