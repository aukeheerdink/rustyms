use itertools::Itertools;

use crate::{modification::Modification, AminoAcid, LinearPeptide, Mass, SequenceElement};

pub fn find_isobaric_sets(
    mass: Mass,
    tolerance_ppm: f64,
    modifications: &[Modification],
) -> IsobaricSetIterator {
    let bounds = (
        mass.value * (1.0 - tolerance_ppm / 1e6),
        mass.value * (1.0 + tolerance_ppm / 1e6),
    );

    // Create the building blocks
    let mut n_term: Vec<(SequenceElement, f64)> = AA
        .iter()
        .flat_map(|aa| {
            let mut options = vec![SequenceElement::new(*aa, None)];
            options.extend(modifications.iter().filter_map(|m| {
                can_be_placed(m, *aa, 0, 1).then(|| SequenceElement {
                    aminoacid: *aa,
                    ambiguous: None,
                    modifications: vec![m.clone()],
                    possible_modifications: Vec::new(),
                })
            }));
            options
        })
        .map(|s| {
            (
                s.clone(),
                s.formula_all().unwrap().monoisotopic_mass().unwrap().value,
            )
        })
        .collect();
    n_term.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut center: Vec<(SequenceElement, f64)> = AA
        .iter()
        .flat_map(|aa| {
            let mut options = vec![SequenceElement::new(*aa, None)];
            options.extend(modifications.iter().filter_map(|m| {
                can_be_placed(m, *aa, 1, 2).then(|| SequenceElement {
                    aminoacid: *aa,
                    ambiguous: None,
                    modifications: vec![m.clone()],
                    possible_modifications: Vec::new(),
                })
            }));
            options
        })
        .map(|s| {
            (
                s.clone(),
                s.formula_all().unwrap().monoisotopic_mass().unwrap().value,
            )
        })
        .collect();
    center.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut c_term: Vec<(SequenceElement, f64)> = AA
        .iter()
        .flat_map(|aa| {
            let mut options = vec![SequenceElement::new(*aa, None)];
            options.extend(modifications.iter().filter_map(|m| {
                can_be_placed(m, *aa, 1, 1).then(|| SequenceElement {
                    aminoacid: *aa,
                    ambiguous: None,
                    modifications: vec![m.clone()],
                    possible_modifications: Vec::new(),
                })
            }));
            options
        })
        .map(|s| {
            (
                s.clone(),
                s.formula_all().unwrap().monoisotopic_mass().unwrap().value,
            )
        })
        .collect();
    c_term.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let lightest = center.iter().fold(f64::INFINITY, |acc, s| s.1.min(acc));

    dbg!(IsobaricSetIterator {
        n_term,
        c_term,
        center,
        lightest,
        bounds,
        state: (None, None, Vec::new()),
    })
}

#[derive(Debug)]
pub struct IsobaricSetIterator {
    n_term: Vec<(SequenceElement, f64)>,
    c_term: Vec<(SequenceElement, f64)>,
    center: Vec<(SequenceElement, f64)>,
    lightest: f64,
    bounds: (f64, f64),
    state: (Option<usize>, Option<usize>, Vec<usize>),
}

impl IsobaricSetIterator {
    fn current_mass(&self) -> f64 {
        let mass = self.state.0.map(|i| self.n_term[i].1).unwrap_or_default()
            + self.state.1.map(|i| self.c_term[i].1).unwrap_or_default()
            + self
                .state
                .2
                .iter()
                .copied()
                .map(|i| self.center[i].1)
                .sum::<f64>();
        //println!("{}\t{}", mass.value, self.peptide());
        mass
    }

    fn mass_fits(&self) -> bool {
        let mass = self.current_mass();
        mass > self.bounds.0 && mass < self.bounds.1
    }

    fn peptide(&self) -> LinearPeptide {
        let mut sequence = Vec::with_capacity(
            self.state.2.len()
                + usize::from(self.state.0.is_some())
                + usize::from(self.state.1.is_some()),
        );
        if let Some(n) = self.state.0.map(|i| self.n_term[i].clone()) {
            sequence.push(n.0);
        }
        sequence.extend(
            self.state
                .2
                .iter()
                .copied()
                .map(|i| self.center[i].0.clone()),
        );
        if let Some(c) = self.state.1.map(|i| self.c_term[i].clone()) {
            sequence.push(c.0);
        }
        LinearPeptide {
            global: Vec::new(),
            labile: Vec::new(),
            n_term: None,
            c_term: None,
            sequence,
            ambiguous_modifications: Vec::new(),
            charge_carriers: None,
        }
    }

    fn scan(&mut self) -> Option<LinearPeptide> {
        println!("Scan");
        let last = self.state.2.last().copied().unwrap_or(0); // Be sure to not retry combination that where already tried
        let prev = self.state.2.len();
        while self.current_mass() < self.bounds.0 - self.lightest {
            self.state.2.push(last);
        }
        if self.state.2.len() > prev {
            println!("Scan added until {} aas", self.state.2.len());
        }
        //dbg!(&self.state.2);
        // See if the naive addition of the first elements worked
        if self.current_mass() <= self.bounds.1 {
            return Some(self.peptide());
        }

        // Now loop over the last elements to see if any SequenceElements fits the mass
        let last = self.state.2.len() - 1;
        let start = self.state.2[last] + 1;
        for n in start..self.center.len() {
            self.state.2[last] = n;
            let mass = self.current_mass();
            if mass < self.bounds.0 - self.lightest {
                return self.scan(); // Too light try to add more AAs
            }

            if mass > self.bounds.0 && mass < self.bounds.1 {
                return Some(self.peptide());
            }
        }
        println!("Scanned last level");
        None
    }
}

impl Iterator for IsobaricSetIterator {
    type Item = LinearPeptide;
    fn next(&mut self) -> Option<Self::Item> {
        println!(
            "{:?}[{}]{:?}",
            self.state.0,
            self.state.2.iter().map(ToString::to_string).join(","),
            self.state.1,
        );
        println!("Whole new element");
        loop {
            // Check the state (a list of selected pieces)
            if let Some(pep) = self.scan() {
                return Some(pep);
            }
            //dbg!(&self.state.2);
            // No match was found do a prune back as many levels as needed and do the scan again
            self.state.2.pop();
            // If we reach rock bottom give up (not sure this works we might need to have tried all options for this level first)
            if self.state.2.is_empty() {
                return None;
            }
            let last = self.state.2.len() - 1;
            self.state.2[last] += 1;
            println!("Pop");
            while self.state.2[self.state.2.len() - 1] <= self.center.len() {
                self.state.2.pop();
                println!("Pop one more");
                let last = self.state.2.len() - 1;
                self.state.2[last] += 1;
            }
            // If we reach rock bottom give up (not sure this works we might need to have tried all options for this level first)
            if self.state.2.is_empty() {
                return None;
            }
        }
    }
}

/// Enforce the placement rules of predefined modifications.
fn can_be_placed(modification: &Modification, aa: AminoAcid, index: usize, length: usize) -> bool {
    if let Modification::Predefined(_, rules, _, _, _) = modification {
        rules.is_empty()
            || rules.iter().any(|rule| {
                rule.is_possible(
                    &SequenceElement {
                        aminoacid: aa,
                        modifications: Vec::new(),
                        possible_modifications: Vec::new(),
                        ambiguous: None,
                    },
                    index,
                    length,
                )
            })
    } else {
        true
    }
}

const AA: &[AminoAcid] = &[
    AminoAcid::Glycine,
    AminoAcid::Alanine,
    AminoAcid::Arginine,
    AminoAcid::Asparagine,
    AminoAcid::AsparticAcid,
    AminoAcid::Cysteine,
    AminoAcid::Glutamine,
    AminoAcid::GlutamicAcid,
    AminoAcid::Histidine,
    AminoAcid::AmbiguousLeucine,
    AminoAcid::Lysine,
    AminoAcid::Methionine,
    AminoAcid::Phenylalanine,
    AminoAcid::Proline,
    AminoAcid::Serine,
    AminoAcid::Threonine,
    AminoAcid::Tryptophan,
    AminoAcid::Tyrosine,
    AminoAcid::Valine,
    AminoAcid::Selenocysteine,
    AminoAcid::Pyrrolysine,
];

#[cfg(test)]
mod tests {
    use crate::ComplexPeptide;

    use super::*;
    #[test]
    fn simple_isobaric_sets() {
        let pep = ComplexPeptide::pro_forma("AG").unwrap().assume_linear();
        let sets: Vec<LinearPeptide> = find_isobaric_sets(
            pep.bare_formula().unwrap().monoisotopic_mass().unwrap(),
            10.0,
            &[],
        )
        .collect();
        assert_eq!(
            &sets,
            &[
                ComplexPeptide::pro_forma("AG").unwrap().assume_linear(),
                ComplexPeptide::pro_forma("Q").unwrap().assume_linear(),
            ]
        );
    }
}