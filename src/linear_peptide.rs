#![warn(dead_code)]

use std::{fmt::Display, ops::RangeBounds};

use crate::{
    error::CustomError,
    modification::{AmbiguousModification, GlobalModification, GnoComposition, ReturnModification},
    molecular_charge::MolecularCharge,
    Element, MolecularFormula, Multi, MultiChemical, SequenceElement,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use uom::num_traits::Zero;

use crate::{
    aminoacids::AminoAcid, fragment::Fragment, fragment::FragmentType, modification::Modification,
    system::f64::*, Chemical, Model,
};

/// A peptide with all data as provided by pro forma. Preferably generated by using the [`crate::ComplexPeptide::pro_forma`] function.
#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, Hash)]
pub struct LinearPeptide {
    /// Global isotope modifications, saved as the element and the species that
    /// all occurrence of that element will consist of. Eg (N, 15) will make
    /// all occurring nitrogens be isotope 15.
    global: Vec<(Element, Option<u16>)>,
    /// Labile modifications, which will not be found in the actual spectrum.
    pub labile: Vec<Modification>,
    /// N terminal modification
    pub n_term: Option<Modification>,
    /// C terminal modification
    pub c_term: Option<Modification>,
    /// The sequence of this peptide (includes local modifications)
    pub sequence: Vec<SequenceElement>,
    /// For each ambiguous modification list all possible positions it can be placed on.
    /// Indexed by the ambiguous modification id.
    pub ambiguous_modifications: Vec<Vec<usize>>,
    /// The adduct ions, if specified
    pub charge_carriers: Option<MolecularCharge>,
}

/// Builder style methods to create a [`LinearPeptide`]
impl LinearPeptide {
    /// Create a new [`LinearPeptide`], if you want an empty peptide look at [`LinearPeptide::default`].
    /// Potentially the collect() or into() methods can be useful as well.
    #[must_use]
    pub fn new(sequence: impl IntoIterator<Item = SequenceElement>) -> Self {
        sequence.into_iter().collect()
    }

    /// Add global isotope modifications, if any is invalid it returns None
    #[must_use]
    pub fn global(
        mut self,
        global: impl IntoIterator<Item = (Element, Option<u16>)>,
    ) -> Option<Self> {
        for modification in global {
            if modification.0.is_valid(modification.1) {
                self.global.push(modification);
            } else {
                return None;
            }
        }
        Some(self)
    }

    /// Add labile modifications
    #[must_use]
    pub fn labile(mut self, labile: impl IntoIterator<Item = Modification>) -> Self {
        self.labile.extend(labile);
        self
    }

    /// Add the N terminal modification
    #[must_use]
    pub fn n_term(mut self, term: Option<Modification>) -> Self {
        self.n_term = term;
        self
    }

    /// Add the C terminal modification
    #[must_use]
    pub fn c_term(mut self, term: Option<Modification>) -> Self {
        self.c_term = term;
        self
    }

    /// Add the charge carriers
    #[must_use]
    pub fn charge_carriers(mut self, charge: Option<MolecularCharge>) -> Self {
        self.charge_carriers = charge;
        self
    }
}

impl LinearPeptide {
    /// Get the number of amino acids making up this peptide
    pub fn len(&self) -> usize {
        self.sequence.len()
    }

    /// Check if there are any amino acids in this peptide
    pub fn is_empty(&self) -> bool {
        self.sequence.is_empty()
    }

    /// The mass of the N terminal modifications. The global isotope modifications are NOT applied.
    pub fn get_n_term(&self) -> MolecularFormula {
        self.n_term.as_ref().map_or_else(
            || molecular_formula!(H 1).unwrap(),
            |m| molecular_formula!(H 1).unwrap() + m.formula(),
        )
    }

    /// The mass of the C terminal modifications. The global isotope modifications are NOT applied.
    pub fn get_c_term(&self) -> MolecularFormula {
        self.c_term.as_ref().map_or_else(
            || molecular_formula!(H 1 O 1).unwrap(),
            |m| molecular_formula!(H 1 O 1).unwrap() + m.formula(),
        )
    }

    /// Get the global isotope modifications
    pub fn get_global(&self) -> &[(Element, Option<u16>)] {
        &self.global
    }

    /// Get the reverse of this peptide
    #[must_use]
    pub fn reverse(&self) -> Self {
        Self {
            n_term: self.c_term.clone(),
            c_term: self.n_term.clone(),
            sequence: self.sequence.clone().into_iter().rev().collect(),
            ambiguous_modifications: self
                .ambiguous_modifications
                .clone()
                .into_iter()
                .map(|m| m.into_iter().map(|loc| self.len() - loc).collect())
                .collect(),
            ..self.clone()
        }
    }

    /// Assume that the underlying peptide does not use fancy parts of the Pro Forma spec. This is the common lower bound for support in all functions of rustyms.
    /// If you want to be even more strict on the kind of peptides you want to take take a look at [`Self::assume_very_simple`].
    /// # Panics
    /// When any of these functions are used:
    /// * Labile modifications
    /// * Global isotope modifications
    /// * Charge carriers, use of charged ions apart from protons
    /// * or when the sequence is empty.
    #[must_use]
    pub fn assume_simple(self) -> Self {
        assert!(
            self.labile.is_empty(),
            "A simple linear peptide was assumed, but it has labile modifications"
        );
        assert!(
            self.global.is_empty(),
            "A simple linear peptide was assumed, but it has global isotope modifications"
        );
        assert!(
            self.charge_carriers.is_none(),
            "A simple linear peptide was assumed, but it has specified charged ions"
        );
        assert!(
            !self.sequence.is_empty(),
            "A simple linear peptide was assumed, but it has no sequence"
        );
        self
    }

    /// Assume that the underlying peptide does not use fancy parts of the Pro Forma spec.
    /// # Panics
    /// When any of these functions are used:
    /// * Ambiguous modifications
    /// * Labile modifications
    /// * Global isotope modifications
    /// * Ambiguous amino acids (B/Z)
    /// * Ambiguous amino acid sequence `(?AA)`
    /// * Charge carriers, use of charged ions apart from protons
    /// * or when the sequence is empty.
    #[must_use]
    pub fn assume_very_simple(self) -> Self {
        assert!(
            self.ambiguous_modifications.is_empty(),
            "A simple linear peptide was assumed, but it has ambiguous modifications"
        );
        assert!(
            self.labile.is_empty(),
            "A simple linear peptide was assumed, but it has labile modifications"
        );
        assert!(
            self.global.is_empty(),
            "A simple linear peptide was assumed, but it has global isotope modifications"
        );
        assert!(
            !self
                .sequence
                .iter()
                .any(|seq| seq.aminoacid == AminoAcid::B || seq.aminoacid == AminoAcid::Z),
            "A simple linear peptide was assumed, but it has ambiguous amino acids (B/Z)"
        );
        assert!(
            !self.sequence.iter().any(|seq| seq.ambiguous.is_some()),
            "A simple linear peptide was assumed, but it has ambiguous amino acids `(?AA)`"
        );
        assert!(
            self.charge_carriers.is_none(),
            "A simple linear peptide was assumed, but it has specified charged ions"
        );
        assert!(
            !self.sequence.is_empty(),
            "A simple linear peptide was assumed, but it has no sequence"
        );
        self
    }

    pub(crate) fn enforce_modification_rules(&self) -> Result<(), CustomError> {
        for (index, element) in self.sequence.iter().enumerate() {
            element.enforce_modification_rules(index, self.sequence.len())?;
        }
        Ok(())
    }

    /// Generate all possible patterns for the ambiguous positions (Mass, String:Label).
    /// It always contains at least one pattern (being (base mass, "")).
    /// The global isotope modifications are NOT applied.
    fn ambiguous_patterns(
        &self,
        aa_range: impl RangeBounds<usize>,
        aa: &[SequenceElement],
        index: usize,
        base: MolecularFormula,
    ) -> Vec<(MolecularFormula, String)> {
        let result = self
            .ambiguous_modifications
            .iter()
            .enumerate()
            .fold(vec![Vec::new()], |acc, (id, possibilities)| {
                acc.into_iter()
                    .flat_map(|path| {
                        let mut path_clone = path.clone();
                        let options = possibilities
                            .iter()
                            .filter(|pos| aa_range.contains(pos))
                            .map(move |pos| {
                                let mut new = path.clone();
                                new.push((id, *pos));
                                new
                            });
                        options.chain(
                            possibilities
                                .iter()
                                .find(|pos| !aa_range.contains(pos))
                                .map(move |pos| {
                                    path_clone.push((id, *pos));
                                    path_clone
                                }),
                        )
                    })
                    .collect()
            })
            .into_iter()
            .flat_map(|pattern| {
                let ambiguous_local = pattern
                    .iter()
                    .filter_map(|(id, pos)| (*pos == index).then_some(id))
                    .collect::<Vec<_>>();
                aa.iter()
                    .enumerate()
                    .fold(Multi::default(), |acc, (index, aa)| {
                        acc * aa.formulas(
                            &pattern
                                .clone()
                                .iter()
                                .copied()
                                .filter_map(|(id, pos)| (pos == index).then_some(id))
                                .collect_vec(),
                        )
                    })
                    .iter()
                    .map(|m| {
                        &base
                            + m
                            + self.sequence[index]
                                .possible_modifications
                                .iter()
                                .filter(|&am| ambiguous_local.contains(&&am.id))
                                .map(|am| am.modification.formula())
                                .sum::<MolecularFormula>()
                    })
                    .map(|m| {
                        (
                            m,
                            pattern.iter().fold(String::new(), |acc, (id, pos)| {
                                format!(
                                    "{acc}{}{}@{}",
                                    if acc.is_empty() { "" } else { "," },
                                    &self.sequence[index]
                                        .possible_modifications
                                        .iter()
                                        .find(|am| am.id == *id)
                                        .map_or(String::new(), |v| v
                                            .group
                                            .as_ref()
                                            .map_or(id.to_string(), |g| g.0.clone())),
                                    pos + 1
                                )
                            }),
                        )
                    })
                    .collect_vec()
            })
            .collect::<Vec<(MolecularFormula, String)>>();
        if result.is_empty() {
            vec![(base, String::new())]
        } else {
            result
        }
    }

    /// Gives all the formulas for the whole peptide with no C and N terminal modifications. With the global isotope modifications applied.
    pub fn bare_formulas(&self) -> Multi<MolecularFormula> {
        let mut formulas = Multi::default();
        let mut placed = vec![false; self.ambiguous_modifications.len()];
        for pos in &self.sequence {
            formulas *= pos.formulas_greedy(&mut placed);
        }

        formulas
            .iter()
            .map(|f| {
                f.with_global_isotope_modifications(&self.global)
                    .expect("Invalid global isotope modification in bare_formulas")
            })
            .collect()
    }

    /// Generate the theoretical fragments for this peptide, with the given maximal charge of the fragments, and the given model.
    /// With the global isotope modifications applied.
    ///
    /// # Panics
    /// If `max_charge` outside the range `1..=u64::MAX`.
    pub fn generate_theoretical_fragments(
        &self,
        max_charge: Charge,
        model: &Model,
        peptide_index: usize,
    ) -> Vec<Fragment> {
        assert!(max_charge.value >= 1.0);
        assert!(max_charge.value <= u64::MAX as f64);

        let default_charge = MolecularCharge::proton(max_charge.value as isize);
        let charge_carriers = self.charge_carriers.as_ref().unwrap_or(&default_charge);

        let mut output = Vec::with_capacity(20 * self.sequence.len() + 75); // Empirically derived required size of the buffer (Derived from Hecklib)
        for index in 0..self.sequence.len() {
            let n_term = self.ambiguous_patterns(
                0..=index,
                &self.sequence[0..index],
                index,
                self.get_n_term(),
            );

            let c_term = self.ambiguous_patterns(
                index..self.sequence.len(),
                &self.sequence[index + 1..self.sequence.len()],
                index,
                self.get_c_term(),
            );

            output.append(
                &mut self.sequence[index].aminoacid.fragments(
                    &n_term,
                    &c_term,
                    &self.sequence[index]
                        .modifications
                        .iter()
                        .map(Chemical::formula)
                        .sum(),
                    charge_carriers,
                    index,
                    self.sequence.len(),
                    &model.ions(index, self.sequence.len()),
                    peptide_index,
                ),
            );
        }
        for fragment in &mut output {
            fragment.formula = fragment
                .formula
                .with_global_isotope_modifications(&self.global)
                .expect("Invalid global isotope modification");
        }

        // Generate precursor peak
        output.extend(self.formulas().iter().flat_map(|m| {
            Fragment::new(
                m.clone(),
                Charge::zero(),
                peptide_index,
                FragmentType::precursor,
                String::new(),
            )
            .with_charge(charge_carriers)
            .with_neutral_losses(&model.precursor)
        }));

        // Add glycan fragmentation to all peptide fragments
        // Assuming that only one glycan can ever fragment at the same time,
        // and that no peptide fragmentation occurs during glycan fragmentation
        for (sequence_index, position) in self.sequence.iter().enumerate() {
            for modification in &position.modifications {
                if let Modification::GlycanStructure(glycan) = modification {
                    output.extend(
                        glycan
                            .clone()
                            .determine_positions()
                            .generate_theoretical_fragments(
                                model,
                                peptide_index,
                                charge_carriers,
                                &self.formulas(),
                                (position.aminoacid, sequence_index),
                            ),
                    );
                } else if let Modification::Gno(GnoComposition::Structure(glycan), _) = modification
                {
                    output.extend(
                        glycan
                            .clone()
                            .determine_positions()
                            .generate_theoretical_fragments(
                                model,
                                peptide_index,
                                charge_carriers,
                                &self.formulas(),
                                (position.aminoacid, sequence_index),
                            ),
                    );
                }
            }
        }

        output
    }

    /// Apply a global modification if this is a global isotope modification with invalid isotopes it returns false
    #[must_use]
    pub(crate) fn apply_global_modifications(
        &mut self,
        global_modifications: &[GlobalModification],
    ) -> bool {
        let length = self.len();
        for modification in global_modifications {
            match modification {
                GlobalModification::Fixed(aa, modification) => {
                    for (_, seq) in self.sequence.iter_mut().enumerate().filter(|(index, seq)| {
                        seq.aminoacid == *aa && modification.is_possible(seq, *index, length)
                    }) {
                        seq.modifications.push(modification.clone());
                    }
                }
                GlobalModification::Free(modification) => {
                    for (_, seq) in self
                        .sequence
                        .iter_mut()
                        .enumerate()
                        .filter(|(index, seq)| modification.is_possible(seq, *index, length))
                    {
                        seq.modifications.push(modification.clone());
                    }
                }
                GlobalModification::Isotope(el, isotope) if el.is_valid(*isotope) => {
                    self.global.push((*el, *isotope));
                }
                GlobalModification::Isotope(..) => return false,
            }
        }
        true
    }

    /// Place all global unknown positions at all possible locations as ambiguous modifications
    pub(crate) fn apply_unknown_position_modification(
        &mut self,
        unknown_position_modifications: &[Modification],
    ) {
        for modification in unknown_position_modifications {
            let id = self.ambiguous_modifications.len();
            let length = self.len();
            #[allow(clippy::unnecessary_filter_map)]
            // Side effects so the lint does not apply here
            self.ambiguous_modifications.push(
                (0..length)
                    .filter_map(|i| {
                        if modification.is_possible(&self.sequence[i], i, length) {
                            self.sequence[i]
                                .possible_modifications
                                .push(AmbiguousModification {
                                    id,
                                    modification: modification.clone(),
                                    localisation_score: None,
                                    group: None,
                                });
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect(),
            );
        }
    }
    /// Place all ranged unknown positions at all possible locations as ambiguous modifications
    pub(crate) fn apply_ranged_unknown_position_modification(
        &mut self,
        ranged_unknown_position_modifications: &[(usize, usize, ReturnModification)],
        ambiguous_lookup: &[(Option<String>, Option<Modification>)],
    ) {
        for (start, end, ret_modification) in ranged_unknown_position_modifications {
            let (id, modification, score, group) = match ret_modification {
                ReturnModification::Defined(def) => {
                    self.ambiguous_modifications.push(Vec::new());
                    (
                        self.ambiguous_modifications.len() - 1,
                        def.clone(),
                        None,
                        None,
                    )
                }
                ReturnModification::Preferred(i, score) => {
                    if *i >= self.ambiguous_modifications.len() {
                        self.ambiguous_modifications.push(Vec::new());
                    }
                    (
                        *i,
                        ambiguous_lookup[*i].1.clone().unwrap(),
                        *score,
                        Some((ambiguous_lookup[*i].0.clone().unwrap(), true)), // TODO: now all possible location in the range are listed as preferred
                    )
                }
                ReturnModification::Referenced(i, score) => {
                    if *i >= self.ambiguous_modifications.len() {
                        self.ambiguous_modifications.push(Vec::new());
                    }
                    (
                        *i,
                        ambiguous_lookup[*i].1.clone().unwrap(),
                        *score,
                        Some((ambiguous_lookup[*i].0.clone().unwrap(), false)),
                    )
                }
            };
            let length = self.len();
            #[allow(clippy::unnecessary_filter_map)]
            // Side effects so the lint does not apply here
            let positions = (*start..=*end)
                .filter_map(|i| {
                    if modification.is_possible(&self.sequence[i], i, length) {
                        self.sequence[i]
                            .possible_modifications
                            .push(AmbiguousModification {
                                id,
                                modification: modification.clone(),
                                localisation_score: None,
                                group: group.clone(),
                            });
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect_vec();
            if let Some(score) = score {
                let individual_score = score / positions.len() as f64;
                for pos in &positions {
                    self.sequence[*pos]
                        .possible_modifications
                        .last_mut()
                        .unwrap()
                        .localisation_score = Some(individual_score);
                }
            }
            self.ambiguous_modifications[id].extend(positions);
        }
    }
}

impl MultiChemical for LinearPeptide {
    /// Gives the formulas for the whole peptide. With the global isotope modifications applied. (Any B/Z will result in multiple possible formulas.)
    fn formulas(&self) -> Multi<MolecularFormula> {
        let mut formulas: Multi<MolecularFormula> =
            vec![self.get_n_term() + self.get_c_term()].into();
        let mut placed = vec![false; self.ambiguous_modifications.len()];
        for pos in &self.sequence {
            formulas *= pos.formulas_greedy(&mut placed);
        }

        formulas
            .iter()
            .map(|f| f.with_global_isotope_modifications(&self.global).expect("Global isotope modification invalid in determination of all formulas for a peptide"))
            .collect()
    }
}

impl Display for LinearPeptide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut placed = Vec::new();
        if let Some(m) = &self.n_term {
            write!(f, "[{m}]-")?;
        }
        let mut last_ambiguous = None;
        for position in &self.sequence {
            placed.extend(position.display(f, &placed, last_ambiguous)?);
            last_ambiguous = position.ambiguous;
        }
        if last_ambiguous.is_some() {
            write!(f, ")")?;
        }
        if let Some(m) = &self.c_term {
            write!(f, "-[{m}]")?;
        }
        Ok(())
    }
}

impl<Collection, Item> From<Collection> for LinearPeptide
where
    Collection: IntoIterator<Item = Item>,
    Item: Into<SequenceElement>,
{
    fn from(value: Collection) -> Self {
        Self {
            global: Vec::new(),
            labile: Vec::new(),
            n_term: None,
            c_term: None,
            sequence: value.into_iter().map(std::convert::Into::into).collect(),
            ambiguous_modifications: Vec::new(),
            charge_carriers: None,
        }
    }
}

impl<Item> FromIterator<Item> for LinearPeptide
where
    Item: Into<SequenceElement>,
{
    fn from_iter<T: IntoIterator<Item = Item>>(iter: T) -> Self {
        Self::from(iter)
    }
}

// TODO: implement indexing with range and usize for LinearPeptide
