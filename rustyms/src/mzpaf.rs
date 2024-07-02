//! WIP: mzPAF parser
use std::{ops::Range, sync::OnceLock};

use crate::{
    error::{Context, CustomError},
    helper_functions::{explain_number_error, next_number, Characters, RangeExtension, RangeMaths},
    modification::{Ontology, SimpleModification},
    system::{e, isize::Charge, mz, MassOverCharge},
    AminoAcid, Fragment, MolecularFormula, NeutralLoss, Tolerance,
};

/// Parse a mzPAF peak annotation line (can contain multiple annotations).
/// # Errors
/// When the annotation does not follow the format.
pub fn parse_mzpaf(_line: &str) -> Result<Vec<Fragment>, CustomError> {
    Ok(Vec::new())
}

/// Parse a single mzPAF peak annotation.
/// # Errors
/// When the annotation does not follow the format.
fn parse_annotation(line: &str, range: Range<usize>) -> Result<Fragment, CustomError> {
    // Parse &
    let (left_range, _auxiliary) = if line[range.clone()].starts_with('&') {
        (range.add_start(1_usize), true)
    } else {
        (range.clone(), false)
    };
    let (left_range, _analyte_number) = parse_analyte_number(line, left_range)?;
    let (offset, _ion) = parse_ion(line, left_range)?;
    let (offset, _neutral_losses) = parse_neutral_loss(line, range.add_start(offset))?;
    // Parse isotopes
    let (offset, _charge) = parse_charge(line, range.add_start(offset))?;
    // Parse adduct type
    let (offset, _deviation) = parse_deviation(line, range.add_start(offset))?;
    // Parse confidence
    if offset == range.len() {
        Ok(Fragment::default())
    } else {
        Err(CustomError::error(
            "Invalid mzPAF annotation",
            "These characters could not be parsed",
            Context::line_range(None, line, range.add_start(offset)),
        ))
    }
}

enum IonType {
    Unknown(Option<usize>),
    MainSeries(char, usize),
    Immonium(AminoAcid, Option<SimpleModification>),
    Internal(usize, usize),
    Named(String),
    Precursor,
    Reporter(MolecularFormula),
    Formula(MolecularFormula),
}

/// Parse a mzPAF analyte number. '1@...'
/// # Errors
/// When the ion is not formatted correctly.
fn parse_analyte_number(
    line: &str,
    range: Range<usize>,
) -> Result<(Range<usize>, Option<usize>), CustomError> {
    next_number::<false, false, usize>(line, range.clone()).map_or_else(
        || Ok((range.clone(), None)),
        |num| {
            if line.chars().nth(num.0) != Some('@') {
                return Err(CustomError::error(
                    "Invalid mzPAF analyte number",
                    "The analyte number should be followed by an at sign '@'",
                    Context::line(None, line, num.0, 1),
                ));
            }
            Ok((
                range.add_start(num.0 + 1),
                Some(num.2.map_err(|err| {
                    CustomError::error(
                        "Invalid mzPAF analyte number",
                        format!("The analyte number number {}", explain_number_error(&err)),
                        Context::line(None, line, 0, num.0),
                    )
                })?),
            ))
        },
    )
}

/// Parse a mzPAF ion.
/// # Errors
/// When the ion is not formatted correctly.
fn parse_ion(line: &str, range: Range<usize>) -> Result<(Characters, IonType), CustomError> {
    match line[range.clone()].chars().next() {
        Some('?') => {
            if let Some(ordinal) =
                next_number::<false, false, usize>(line, range.add_start(1_usize))
            {
                Ok((
                    1 + ordinal.0,
                    IonType::Unknown(Some(ordinal.2.map_err(|err| {
                        CustomError::error(
                            "Invalid mzPAF unknown ion ordinal",
                            format!("The ordinal number {}", explain_number_error(&err)),
                            Context::line(None, line, range.start_index() + 1, ordinal.0),
                        )
                    })?)),
                ))
            } else {
                Ok((1, IonType::Unknown(None)))
            }
        }
        Some(c @ ('a' | 'b' | 'c' | 'x' | 'y' | 'z')) => {
            if let Some(ordinal) =
                next_number::<false, false, usize>(line, range.add_start(1_usize))
            {
                Ok((
                    1 + ordinal.0,
                    IonType::MainSeries(
                        c,
                        ordinal.2.map_err(|err| {
                            CustomError::error(
                                "Invalid mzPAF unknown ion ordinal",
                                format!("The ordinal number {}", explain_number_error(&err)),
                                Context::line(None, line, range.start_index() + 1, ordinal.0),
                            )
                        })?,
                    ),
                ))
                // TODO: potentially followed by a pro forma sequence in {}
            } else {
                Err(CustomError::error(
                    "Invalid mzPAF main series ion ordinal",
                    "For a main series ion the ordinal should be provided, like 'a12'",
                    Context::line(None, line, range.start_index(), 1),
                ))
            }
        }
        Some('I') => {
            let amino_acid = line[range.clone()].chars().nth(1).ok_or_else(|| {
                CustomError::error(
                    "Invalid mzPAF immonium",
                    "The source amino acid for this immonium ion should be present like 'IA'",
                    Context::line(None, line, range.start_index(), 1),
                )
            })?;
            let modification = if line[range.clone()].chars().nth(2) == Some('[') {
                let first = line[range.clone()].char_indices().nth(3).unwrap().0;
                let last = line[range.clone()]
                    .char_indices()
                    .skip(3)
                    .take_while(|(_, c)| *c != ']')
                    .last()
                    .unwrap();
                Some((
                    last.0 + last.1.len_utf8() - first,
                    Ontology::Unimod
                        .find_name(
                            &line[range.clone()][first..last.0 + last.1.len_utf8()],
                            None,
                        )
                        .ok_or_else(|| {
                            Ontology::Unimod.find_closest(
                                &line[range.clone()][first..last.0 + last.1.len_utf8()],
                                None,
                            )
                        })?,
                ))
            } else {
                None
            };
            Ok((
                2 + modification.as_ref().map_or(0, |m| m.0),
                IonType::Immonium(
                    AminoAcid::try_from(amino_acid).map_err(|()| {
                        CustomError::error(
                            "Invalid mzPAF immonium ion",
                            "The provided amino acid is not a known amino acid",
                            Context::line(None, line, range.start_index() + 1, 1),
                        )
                    })?,
                    modification.map(|m| m.1),
                ),
            ))
        }
        Some('m') => {
            let first_ordinal = next_number::<false, false, usize>(line, range.add_start(1_usize))
                .ok_or_else(|| {
                    CustomError::error(
                        "Invalid mzPAF internal ion first ordinal",
                        "The first ordinal for an internal ion should be present",
                        Context::line(None, line, range.start_index(), 1),
                    )
                })?;
            if line[range.clone()].chars().nth(first_ordinal.0) != Some(':') {
                return Err(CustomError::error(
                    "Invalid mzPAF internal ion ordinal separator",
                    "The internal ion ordinal separator should be a colon ':', like 'm4:6'",
                    Context::line(None, line, range.start_index() + 1 + first_ordinal.0, 1),
                ));
            }
            assert!(
                line[range.clone()].chars().nth(first_ordinal.0) == Some(':'),
                "Needs to be separated by colon"
            );
            let second_ordinal = next_number::<false, false, usize>(
                line,
                range.add_start(2 + first_ordinal.0 as isize),
            )
            .ok_or_else(|| {
                CustomError::error(
                    "Invalid mzPAF internal ion second ordinal",
                    "The second ordinal for an internal ion should be present",
                    Context::line(None, line, range.start_index() + 1 + first_ordinal.0, 1),
                )
            })?;
            let first_location = first_ordinal.2.map_err(|err| {
                CustomError::error(
                    "Invalid mzPAF internal ion first ordinal",
                    format!("The ordinal number {}", explain_number_error(&err)),
                    Context::line(None, line, range.start_index() + 1, first_ordinal.0),
                )
            })?;
            let second_location = second_ordinal.2.map_err(|err| {
                CustomError::error(
                    "Invalid mzPAF internal ion second ordinal",
                    format!("The ordinal number {}", explain_number_error(&err)),
                    Context::line(
                        None,
                        line,
                        range.start_index() + 2 + first_ordinal.0,
                        second_ordinal.0,
                    ),
                )
            })?;
            Ok((
                2 + first_ordinal.0 + second_ordinal.0,
                IonType::Internal(first_location, second_location),
            ))
        }
        Some('_') => {
            // Format less strings
            // TODO: Potentially recognise the following as known contaminants:
            // 0@_{y1(R)}
            // 0@_{a2(LP)}
            // 0@_{b2(LP)}

            let (len, name) = if line[range.clone()].chars().nth(1) == Some('{') {
                let first = line[range.clone()].char_indices().nth(2).unwrap().0;
                let last = line[range.clone()]
                    .char_indices()
                    .skip(2)
                    .take_while(|(_, c)| *c != '}')
                    .last()
                    .unwrap();
                Ok((
                    last.0 + last.1.len_utf8() - first,
                    &line[range][first..last.0 + last.1.len_utf8()],
                ))
            } else {
                Err(CustomError::error(
                    "Invalid mzPAF named compound",
                    "A named compound must be named with curly braces '{}' after the '_'",
                    Context::line(None, line, range.start_index(), 1),
                ))
            }?;
            Ok((3 + len, IonType::Named(name.to_string())))
        }
        Some('p') => Ok((1, IonType::Precursor)),
        Some('r') => {
            // Same name as neutral losses
            let (len, name) = if line[range.clone()].chars().nth(1) == Some('[') {
                let first = line[range.clone()].char_indices().nth(2).unwrap().0;
                let last = line[range.clone()]
                    .char_indices()
                    .skip(2)
                    .take_while(|(_, c)| *c != ']')
                    .last()
                    .unwrap();
                Ok((
                    last.0 + last.1.len_utf8() - first,
                    &line[range.clone()][first..last.0 + last.1.len_utf8()],
                ))
            } else {
                Err(CustomError::error(
                    "Invalid mzPAF reporter ion",
                    "A reporter ion must be named with square braces '[]' after the 'r'",
                    Context::line(None, line, range.start_index(), 1),
                ))
            }?;
            mz_paf_named_molecules()
                .iter()
                .find_map(|n| (n.0 == name).then_some(n.1.clone()))
                .map_or_else(
                    || {
                        Err(CustomError::error(
                            "Unknown mzPAF named reporter ion",
                            "Unknown name",
                            Context::line(None, line, range.start_index() + 1, len),
                        ))
                    },
                    |formula| Ok((3 + len, IonType::Reporter(formula))),
                )
        }
        Some('f') => {
            // Simple formula
            let formula_range = if line[range.clone()].chars().nth(1) == Some('{') {
                let first = line[range.clone()].char_indices().nth(2).unwrap().0;
                let last = line[range.clone()]
                    .char_indices()
                    .skip(2)
                    .take_while(|(_, c)| *c != '}')
                    .last()
                    .unwrap();
                Ok(range.start_index() + first..range.start_index() + last.0 + last.1.len_utf8())
            } else {
                Err(CustomError::error(
                    "Invalid mzPAF formula",
                    "A formula must have the formula defined with curly braces '{}' after the 'f'",
                    Context::line(None, line, range.start_index(), 1),
                ))
            }?;
            let formula = MolecularFormula::from_mz_paf(line, formula_range.clone())?;

            Ok((3 + formula_range.len(), IonType::Formula(formula)))
        }
        Some('s') => todo!(), // TODO: return as Formula
        Some(_) => Err(CustomError::error(
            "Invalid ion",
            "An ion cannot start with this character",
            Context::line(None, line, range.start, 1),
        )),
        None => Err(CustomError::error(
            "Invalid ion",
            "An ion cannot be an empty string",
            Context::line_range(None, line, range),
        )),
    }
}

fn parse_neutral_loss(
    line: &str,
    range: Range<usize>,
) -> Result<(Characters, Vec<NeutralLoss>), CustomError> {
    let mut offset = 0;
    let mut neutral_losses = Vec::new();
    while let Some(c @ ('-' | '+')) = line[range.clone()].chars().nth(offset) {
        if line[range.clone()].chars().nth(1) == Some('[') {
            let first = line[range.clone()].char_indices().nth(2).unwrap().0;
            let last = line[range.clone()]
                .char_indices()
                .skip(2)
                .take_while(|(_, c)| *c != ']')
                .last()
                .unwrap();
            //Ok(first..last.0 + last.1.len_utf8());
            let name = line[first..last.0 + last.1.len_utf8()].to_ascii_lowercase();

            offset += 1 + last.0 + last.1.len_utf8() - first;

            if let Some(formula) = mz_paf_named_molecules()
                .iter()
                .find_map(|n| (n.0 == name).then_some(n.1.clone()))
            {
                neutral_losses.push(match c {
                    '+' => NeutralLoss::Gain(formula),
                    '-' => NeutralLoss::Loss(formula),
                    _ => unreachable!(),
                });
            } else {
                return Err(CustomError::error(
                    "Unknown mzPAF named neutral loss",
                    "Unknown name",
                    Context::line(None, line, offset - name.len() - 1, name.len()),
                ));
            }
        } else {
            let first = line[range.clone()].char_indices().nth(1).unwrap().0;
            let last = line[range.clone()]
                .char_indices()
                .skip(2)
                .take_while(|(_, c)| c.is_ascii_alphanumeric())
                .last()
                .unwrap();
            let formula = MolecularFormula::from_mz_paf(line, first..last.0 + last.1.len_utf8())?;
            neutral_losses.push(match c {
                '+' => NeutralLoss::Gain(formula),
                '-' => NeutralLoss::Loss(formula),
                _ => unreachable!(),
            });
            offset += 1 + last.0 + last.1.len_utf8() - first;
        }
    }
    Ok((offset, neutral_losses))
}

fn parse_charge(line: &str, range: Range<usize>) -> Result<(Characters, Charge), CustomError> {
    if line[range.clone()].starts_with('^') {
        let charge =
            next_number::<false, false, u32>(line, range.add_start(1_usize)).ok_or_else(|| {
                CustomError::error(
                    "Invalid mzPAF charge",
                    "The number after the charge symbol should be present, eg '^2'.",
                    Context::line(None, line, range.start_index(), 1),
                )
            })?;
        Ok((
            charge.0 + 1,
            Charge::new::<e>(charge.2.map_err(|err| {
                CustomError::error(
                    "Invalid mzPAF charge",
                    format!("The charge number {}", explain_number_error(&err)),
                    Context::line(None, line, range.start_index() + 1, charge.0),
                )
            })? as isize),
        ))
    } else {
        Ok((0, Charge::new::<e>(1)))
    }
}

// fn parse_adduct(
//     line: &str,
//     range: Range<usize>,
// ) -> Result<(Characters, MolecularFormula), CustomError> {
//     if line[range.clone()].chars().next() == Some('^') {
//         let charge =
//             next_number::<false, false, u32>(line, range.add_start(1)).ok_or_else(|| {
//                 CustomError::error(
//                     "Invalid mzPAF charge",
//                     "The number after the charge symbol should be present, eg '^2'.",
//                     Context::line(None, line, range.start_index(), 1),
//                 )
//             })?;
//         Ok((
//             charge.0 + 1,
//             Charge::new::<e>(charge.2.map_err(|err| {
//                 CustomError::error(
//                     "Invalid mzPAF charge",
//                     format!("The charge number {}", explain_number_error(&err)),
//                     Context::line(None, line, range.start_index() + 1, charge.0),
//                 )
//             })? as isize),
//         ))
//     } else {
//         Ok((0, Charge::new::<e>(1)))
//     }
// }

/// Parse a mzPAF deviation, either a ppm or mz deviation.
/// # Errors
/// When the deviation is not '<number>' or '<number>ppm'.
fn parse_deviation(
    line: &str,
    range: Range<usize>,
) -> Result<(Characters, Option<Tolerance<MassOverCharge>>), CustomError> {
    if line[range.clone()].starts_with('/') {
        let number = next_number::<true, true, f64>(line, range.add_start(1_usize)).ok_or(
            CustomError::error(
                "Invalid mzPAF deviation",
                "A deviation should be a number",
                Context::line_range(None, line, range.start..range.start + 1),
            ),
        )?;
        let deviation = number.2.map_err(|err| {
            CustomError::error(
                "Invalid mzPAF deviation",
                format!("The deviation number {err}",),
                Context::line_range(None, line, range.start + 1..range.start + 1 + number.0),
            )
        })?;
        if line[range.add_start(1 + number.0 as isize)]
            .to_ascii_lowercase()
            .starts_with("ppm")
        {
            Ok((1 + number.0 + 3, Some(Tolerance::new_ppm(deviation))))
        } else {
            Ok((
                1 + number.0,
                Some(Tolerance::new_absolute(MassOverCharge::new::<mz>(
                    deviation,
                ))),
            ))
        }
    } else {
        Ok((0, None))
    }
}

fn mz_paf_named_molecules() -> &'static Vec<(&'static str, MolecularFormula)> {
    MZPAF_NAMED_MOLECULES_CELL.get_or_init(|| {
        vec![
            ("hex", molecular_formula!(C 6 H 10 O 5)),
            ("hexnac", molecular_formula!(C 8 H 13 N 1 O 5)),
            ("dhex", molecular_formula!(C 6 H 10 O 4)),
            ("neuac", molecular_formula!(C 11 H 17 N 1 O 8)),
            ("neugc", molecular_formula!(C 11 H 17 N 1 O 9)),
            ("tmt126", molecular_formula!(C 8 N 1 H 15)),
            ("tmt127n", molecular_formula!(C 8 [15 N 1] H 15)),
            ("tmt127c", molecular_formula!(C 7 [13 C 1] N 1 H 15)),
            ("tmt128n", molecular_formula!(C 7 [13 C 1] [15 N 1] H 15)),
            ("tmt128c", molecular_formula!(C 6 [13 C 2] N 1 H 15)),
            ("tmt129n", molecular_formula!(C 6 [13 C 2] [15 N 1] H 15)),
            ("tmt129c", molecular_formula!(C 5 [13 C 3] N 1 H 15)),
            ("tmt130n", molecular_formula!(C 5 [13 C 3] [15 N 1] H 15)),
            ("tmt130c", molecular_formula!(C 4 [13 C 4] N 1 H 15)),
            ("tmt131n", molecular_formula!(C 4 [13 C 4] [15 N 1] H 15)),
            ("tmt131c", molecular_formula!(C 3 [13 C 5] N 1 H 15)),
            ("tmt132n", molecular_formula!(C 3 [13 C 5] [15 N 1] H 15)),
            ("tmt132c", molecular_formula!(C 2 [13 C 6] N 1 H 15)),
            ("tmt133n", molecular_formula!(C 2 [13 C 6] [15 N 1] H 15)),
            ("tmt133c", molecular_formula!(C 1 [13 C 7] N 1 H 15)),
            ("tmt134n", molecular_formula!(C 1 [13 C 7] [15 N 1] H 15)),
            ("tmt134c", molecular_formula!(C 0 [13 C 8] N 1 H 15)),
            ("tmt135n", molecular_formula!(C 0 [13 C 8] [15 N 1] H 15)),
            ("tmtzero", molecular_formula!(C 12 H 20 N 2 O 2)),
            ("tmtpro_zero", molecular_formula!(C 15 H 25 N 3 O 3)),
            ("tmt2plex", molecular_formula!(C 11 [ 13 C 1] H 20 N 2 O 2)),
            (
                "tmt6plex",
                molecular_formula!(C 8 [13 C 5] H 20 N 1 [ 15 N 1] O 2),
            ),
            (
                "tmtpro",
                molecular_formula!(C 8 [13 C 7] H 25 [15 N 2] N 1 O 3),
            ),
            ("itraq113", molecular_formula!(C 6 N 2 H 12)),
            ("itraq114", molecular_formula!(C 5 [13 C 1] N 2 H 12)),
            (
                "itraq115",
                molecular_formula!(C 5 [13 C 1] N 1 [15 N 1] H 12),
            ),
            (
                "itraq116",
                molecular_formula!(C 4 [13 C 2] N 1 [15 N 1] H 12),
            ),
            (
                "itraq117",
                molecular_formula!(C 3 [13 C 3] N 1 [15 N 1] H 12),
            ),
            ("itraq118", molecular_formula!(C 3 [13 C 3] [15 N 2] H 12)),
            ("itraq119", molecular_formula!(C 4 [13 C 2] [15 N 2] H 12)),
            ("itraq121", molecular_formula!([13 C 6] [15 N 2] H 12)),
            (
                "itraq4plex",
                molecular_formula!(C 4 [13 C 3] H 12 N 1 [15 N 1] O 1),
            ),
            (
                "itraq8plex",
                molecular_formula!(C 7 [13 C 7] H 24 N 3 [15 N 1] O 3),
            ),
            ("tmt126-etd", molecular_formula!(C 7 N 1 H 15)),
            ("tmt127n-etd", molecular_formula!(C 7 [15 N 1] H 15)),
            ("tmt127c-etd", molecular_formula!(C 6 [13 C 1] N 1 H 15)),
            (
                "tmt128n-etd",
                molecular_formula!(C 6 [13 C 1] [15 N 1] H 15),
            ),
            ("tmt128c-etd", molecular_formula!(C 5 [13 C 2] N 1 H 15)),
            (
                "tmt129n-etd",
                molecular_formula!(C 5 [13 C 2] [15 N 1] H 15),
            ),
            ("tmt129c-etd", molecular_formula!(C 4 [13 C 3] N 1 H 15)),
            (
                "tmt130n-etd",
                molecular_formula!(C 4 [13 C 3] [15 N 1] H 15),
            ),
            ("tmt130c-etd", molecular_formula!(C 3 [13 C 4] N 1 H 15)),
            (
                "tmt131n-etd",
                molecular_formula!(C 3 [13 C 4] [15 N 1] H 15),
            ),
            ("tmt131c-etd", molecular_formula!(C 2 [13 C 5] N 1 H 15)),
        ]
    })
}

static MZPAF_NAMED_MOLECULES_CELL: OnceLock<Vec<(&str, MolecularFormula)>> = OnceLock::new();