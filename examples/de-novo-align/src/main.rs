use std::{collections::HashMap, fs::File, io::BufWriter};

use clap::Parser;
use itertools::Itertools;
use rayon::prelude::*;
use rustyms::{
    align::{align, matrix, AlignType},
    csv::write_csv,
    identification::{open_identified_peptides_file, FastaData},
    system::da,
    *,
};

#[derive(Parser)]
struct Cli {
    /// The input identified peptides file
    #[arg(short, long)]
    peptides: String,
    /// The fasta database of known proteins
    #[arg(short, long)]
    database: String,
    /// Where to store the results
    #[arg(long)]
    out_path: String,
}

fn main() {
    let args = Cli::parse();
    let out_file = BufWriter::new(File::create(args.out_path).unwrap());
    let peptides = open_identified_peptides_file(args.peptides, None)
        .unwrap()
        .filter_map(|p| p.ok())
        .collect_vec();
    let database = FastaData::parse_file(args.database).unwrap();

    let alignments: Vec<_> = peptides
        .par_iter()
        .flat_map(|peptide| {
            let alignments = database
                .iter()
                .map(|db| {
                    (
                        db,
                        peptide,
                        align::<4, SemiAmbiguous, SemiAmbiguous>(
                            &db.peptide,
                            peptide.peptide().unwrap(),
                            matrix::BLOSUM62,
                            Tolerance::Absolute(da(0.1)),
                            AlignType::EITHER_GLOBAL,
                        ),
                    )
                })
                .collect_vec();
            let max = alignments
                .iter()
                .max_by(|a, b| a.2.normalised_score().total_cmp(&b.2.normalised_score()))
                .unwrap()
                .2
                .normalised_score();
            let mut alignments = alignments
                .into_iter()
                .filter(|a| a.2.normalised_score() == max)
                .collect_vec();
            if alignments.len() == 1 {
                let (d, p, a) = alignments.pop().unwrap();
                vec![(d, p, a, true)]
            } else {
                alignments
                    .into_iter()
                    .map(|(d, p, a)| (d, p, a, false))
                    .collect_vec()
            }
        })
        .map(|(db, peptide, alignment, unique)| {
            HashMap::from([
                ("Peptide".to_string(), alignment.seq_b().to_string()),
                (
                    "Rawfile".to_string(),
                    peptide
                        .raw_file()
                        .map_or(String::new(), |p| p.to_string_lossy().to_string()),
                ),
                (
                    "De novo score".to_string(),
                    peptide.score.map_or(String::new(), |s| s.to_string()),
                ),
                ("Protein".to_string(), db.id.clone()),
                (
                    "Alignment score".to_string(),
                    alignment.normalised_score().to_string(),
                ),
                ("Unique".to_string(), unique.to_string()),
                ("Start".to_string(), alignment.start_a().to_string()),
                (
                    "End".to_string(),
                    (alignment.start_a() + alignment.len_a()).to_string(),
                ),
                ("Path".to_string(), alignment.short()),
                (
                    "Mass".to_string(),
                    peptide
                        .peptide()
                        .map_or(f64::NAN, |p| {
                            p.clone()
                                .into_unambiguous()
                                .map_or(f64::NAN, |p| p.formula().monoisotopic_mass().value)
                        })
                        .to_string(),
                ),
                (
                    "Z".to_string(),
                    peptide.charge().map_or(0, |c| c.value).to_string(),
                ),
                (
                    "Peptide length".to_string(),
                    peptide.peptide().map_or(0, |p| p.len()).to_string(),
                ),
                (
                    "Scan".to_string(),
                    peptide
                        .scan_indices()
                        .map_or(String::new(), |s| s.into_iter().join(",")),
                ),
                (
                    "Retention time".to_string(),
                    peptide
                        .retention_time()
                        .map_or(f64::NAN, |t| t.value)
                        .to_string(),
                ),
            ])
        })
        .collect();

    write_csv(out_file, alignments).unwrap();
}