# svinst_port: SystemVerilog Modules/Ports Extractor

This tool is a customized version of [svinst](https://github.com/sgherbst/svinst)
that extracts modules, module instantiations, and **ports of the modules**
from SystemVerilog files.

Currently, this repository serves source files only. The Windows executable file
will be included as a part of another project, [DRFront](https://github.com/nfproc/DRFront).

## Purpose

The original version of [svinst](https://github.com/sgherbst/svinst) can extract
a list of module definitions and instantiations. This feature is sufficient for
users to construct a design hierarchy and find a top module. However, one (including
me) might then want to extract a list of ports of the top module. This tool was
developed to meet that demand.

## How to Build

Rust has to be installed to your system to build the project. After checking out
this repository, simply run
>     cargo build --release
to build the project. The executable will be found in the `target/release` folder.

Since the [sv-parser](https://github.com/dalance/sv-parser) library requires large
stack space, building in the debug mode (without `--release`) might lead to a
stack overflow error.

## Usage

In the same way as [svinst](https://github.com/sgherbst/svinst), the `svinst_port`
binary accepts one or more SystemVerilog files as input and prints a YAML, which
represent the module definitions and module instantiation, and port definitions.
Given the sample SystemVerilog file (`sample/sample.sv`), the expected output is
as follows:

>     > svinst_port.exe sample\sample.sv
>     files:
>       - file_name: "sample\\sample.sv"
>         defs:
>           - mod_name: "case1"
>             ports:
>               - port_name: "CLK"
>                 port_dir: "input"
>                 port_width: 1
>               - port_name: "RST"
>                 port_dir: "input"
>                 port_width: 1
>               - port_name: "DATA_IN"
>                 port_dir: "input"
>                 port_width: 32
>               - port_name: "DATA_OUT"
>                 port_dir: "output"
>                 port_width: 8
>               - port_name: "BUSY"
>                 port_dir: "output"
>                 port_width: 1
>             insts:
>               - mod_name: "case2"
>                 inst_name: "c2a"
>               - mod_name: "case2"
>                 inst_name: "c2b"
>           - mod_name: "case2"
>             ports:
>               - port_name: "CLK"
>                 port_dir: "input"
>                 port_width: 1
>               - port_name: "RST"
>                 port_dir: "input"
>                 port_width: 1
>               - port_name: "DIN"
>                 port_dir: "input"
>                 port_width: 16
>               - port_name: "DOUT"
>                 port_dir: "output"
>                 port_width: 4
>               - port_name: "BUSY"
>                 port_dir: "output"
>                 port_width: 1
>             insts: []

## Restrictions

The current version of `svinst_port` has the following restrictions.

A support of packages and classes is omitted in this tool.

For a vector port, the upper and lower bounds of the range has to be an integer
and zero, respectively. A variable, a constant, or an expression is not allowed.

## License

The MIT license is applied. See the LICENSE file for details.