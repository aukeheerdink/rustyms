🦀 Rust:  [![Crates.io](https://img.shields.io/crates/v/rustyms.svg)](https://crates.io/crates/rustyms) [![rustyms documentation](https://docs.rs/rustyms/badge.svg)](https://docs.rs/rustyms)
🐍 Python: [![PyPI version](https://badge.fury.io/py/rustyms.svg)](https://badge.fury.io/py/rustyms) [![Python Docs](https://readthedocs.org/projects/rustyms/badge/?version=latest)](https://rustyms.readthedocs.io/)

# Match those fragments!

A peptide fragmentation matching library for Rust. Built to handle very complex peptides in a sensible way.

## Features

 - Read [ProForma](https://github.com/HUPO-PSI/ProForma) sequences (complete specification supported: 'level 2-ProForma + top-down compliant + cross-linking compliant + glycans compliant + mass spectrum compliant')
 - Generate theoretical fragments with control over the fragmentation model from any ProForma peptidoform/proteoform
   - Generate theoretical fragments for chimeric spectra
   - Generate theoretical fragments for cross-links (also disulfides)
   - Generate theoretical fragments for modifications of unknown position
   - Generate peptide backbone (a, b, c, x, y, and z) and satellite ion fragments (w, d, and v)
   - Generate glycan fragments (B, Y, and internal fragments)
 - Integrated with [mzdata](https://crates.io/crates/mzdata) for reading raw data files
 - Match spectra to the generated fragments
 - [Align peptides based on mass](https://pubs.acs.org/doi/10.1021/acs.jproteome.4c00188)
 - Fast access to the IMGT database of antibody germlines
 - Reading of multiple identified peptide file formats (Fasta, MaxQuant, MSFragger, Novor, OPair, Peaks, and Sage)
 - Exhaustively fuzz tested for reliability (using [cargo-afl](https://crates.io/crates/cargo-afl))
 - Extensive use of [uom](https://docs.rs/uom/latest/uom/) for compile time unit checking
 - Python bindings are provided to several core components of the rustyms library. Go to the [Python documentation](https://rustyms.readthedocs.io/) for more information.

# Folder organisation

## rustyms

This is the main library. This contains all source code, databases (Unimod etc) and example data to run the rustyms library.

## examples

Some examples on how to use the rustyms library are provided here, see the readme file in the examples themselves for more details.

## fuzz

The harness to fuzz test the library for increased stability, see the readme for more details.

## rustyms-py

This Rust library provides python bindings (using pyO3) for rustyms.

## rustyms-generate-databases

Using the `rustyms-generate-databases` the definitions for the databases can be updated. See the readme on the download locations for all databases. Then run `cargo run -p rustyms-generate-databases` (from the root folder of this repository).

## rustyms-generate-imgt

Using the `rustyms-generate-imgt` the definitions for the germlines can be updated. Put the imgt.dat.Z file in the `rustyms-generate-imgt/data` directory and unpack it (this can be downloaded from https://www.imgt.org/download/LIGM-DB/imgt.dat.Z). Then run `cargo run -p rustyms-generate-imgt` (from the root folder of this repository).

# Contributing

Any contribution is welcome (especially adding/fixing documentation as that is very hard to do as main developer).