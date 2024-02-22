//! This crate handles parsing the [IMGT LIGM-DB database](https://www.imgt.org/) into structures compatible with rustyms.
//! It additionally stores all regions and annotations. There are two main ways of selecting germline(s), specified by name
//! [`get_germline`](crate::imgt::get_germline) or by building a query over the data [`Selection`](crate::imgt::Selection).
//!
//! <details><summary>Data present per species</summary>
//!
#![doc = include_str!("germlines/germlines.md")]
//!
//! </details>
//!
//! ```
//! use rustyms::imgt::*;
//! let selection = Selection::default()
//!                           .species([Species::HomoSapiens])
//!                           .chain([ChainType::Heavy])
//!                           .gene([GeneType::V]);
//! let first = selection.germlines().next().unwrap();
//! assert_eq!(first.name(), "IGHV1-2*01");
//! ```

mod fancy;
#[rustfmt::skip]
mod germlines;
mod select;
mod shared;

pub use fancy::*;
#[cfg(feature = "rayon")]
use germlines::par_germlines;
use germlines::{all_germlines, germlines};

pub use select::*;
#[allow(unused_imports)]
pub use shared::*;
