# ast-grep python binding

[![PyPI](https://img.shields.io/pypi/v/ast-grep-py.svg?logo=PyPI)](https://pypi.org/project/ast-grep-py/)
[![Website](https://img.shields.io/badge/ast--grep-Ast--Grep_Website-red?logoColor=red)](https://ast-grep.github.io/)

<p align=center>
  <img src="https://ast-grep.github.io/logo.svg" alt="ast-grep"/>
</p>

## ast-grep

`ast-grep` is a tool for code structural search, lint, and rewriting. 

This crate intends to build a native python binding of ast-grep and provide a python API for programmatic usage.

## Installation

```bash
pip install ast-grep-py
```

## Usage

You can take our tests as examples. For example, [test_simple.py](./tests/test_simple.py) shows how to use ast-grep to search for a pattern in a file.

Please see the [API usage guide](https://ast-grep.github.io/guide/api-usage.html) and [API reference](https://ast-grep.github.io/reference/api.html) for more details.

Other resources include [ast-grep's official site](https://ast-grep.github.io/) and [repository](https://github.com/ast-grep/ast-grep).

## Development

### Setup virtualenv

```shell
python -m venv venv
```

### Activate venv

```shell
source venv/bin/activate
```

### Install `maturin`

```shell
pip install maturin[patchelf]
```

### MacOS: Install `patchelf` and `maturin`

```shell
brew install patchelf
pip install maturin
```

### Build bindings

```shell
maturin develop
```

### Run tests

```shell
pytest
```

All tests files are under [tests](./tests) directory.

## License

This project is licensed under the MIT license.
