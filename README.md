🦀 Rust:  [![Crates.io](https://img.shields.io/crates/v/rustyms.svg)](https://crates.io/crates/rustyms) [![rustyms documentation](https://docs.rs/rustyms/badge.svg)](https://docs.rs/rustyms)
🐍 Python: [![PyPI version](https://badge.fury.io/py/rustyms.svg)](https://badge.fury.io/py/rustyms) [![Python Docs](https://readthedocs.org/projects/rustyms/badge/?version=latest)](https://rustyms.readthedocs.io/)

# Match those fragments!

A peptide fragmentation matching library for rust. Split into multiple smaller modules to help keep things organised.

## Features

 - Read [ProForma](https://github.com/HUPO-PSI/ProForma) sequences (complete specification supported: 'level 2-ProForma + top-down compliant + cross-linking compliant + glycans compliant + mass spectrum compliant')
 - Generate theoretical fragments with control over the fragmentation model from any ProForma peptidoform/proteoform
   - Generate fragments from satellite ions (w, d, and v)
   - Generate glycan fragments
   - Generate theoretical fragments for modifications of unknown position
   - Generate theoretical fragments for chimeric spectra
   - Generate theoretical fragments for cross-links (also disulfides)
 - Integrated with [mzdata](https://crates.io/crates/mzdata) for reading raw data file
 - Match spectra to the generated fragments
 - [Align peptides based on mass](https://pubs.acs.org/doi/10.1021/acs.jproteome.4c00188)
 - Fast access to the IMGT database of antibody germlines
 - Reading of multiple identified peptide file formats (Fasta, MaxQuant, MSFragger, Novor, OPair, Peaks, and Sage)
 - Exhaustively fuzz tested for reliability (using [cargo-afl](https://crates.io/crates/cargo-afl))
 - Extensive use of [uom](https://docs.rs/uom/latest/uom/) for compile time unit checking

## Python bindings

Python bindings are provided to several core components of the rustyms library. Go to the
[Python documentation](https://rustyms.readthedocs.io/) for more information.

# Contributing

Any contribution is welcome (especially adding/fixing documentation as that is very hard to do as main developer).

# IMGT generate

Using the `rustyms-imgt-generate` the definitions for the germlines can be updated. Put the imgt.dat.Z file in the `data` directory and unpack it (this can be downloaded from https://www.imgt.org/download/LIGM-DB/imgt.dat.Z). Then run `cargo run --release -p rustyms-imgt-generate` (from the root folder of this repository).
