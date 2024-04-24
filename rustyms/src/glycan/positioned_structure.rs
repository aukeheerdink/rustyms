//! Handle positioned glycan structures
use std::hash::Hash;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::MonoSaccharide;
use crate::{
    formula::{Chemical, MolecularFormula},
    fragment::{Fragment, FragmentType, GlycanBreakPos, GlycanPosition},
    molecular_charge::MolecularCharge,
    system::usize::Charge,
    AminoAcid, Model, Multi,
};

use crate::uom::num_traits::Zero;

/// Rose tree representation of glycan structure
#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub struct PositionedGlycanStructure {
    pub(super) sugar: MonoSaccharide,
    pub(super) branches: Vec<PositionedGlycanStructure>,
    pub(super) inner_depth: usize,
    pub(super) outer_depth: usize,
    pub(super) branch: Vec<usize>,
}

impl Chemical for PositionedGlycanStructure {
    fn formula(&self) -> MolecularFormula {
        self.sugar.formula()
            + self
                .branches
                .iter()
                .map(Chemical::formula)
                .sum::<MolecularFormula>()
    }
}

impl PositionedGlycanStructure {
    /// Generate all theoretical fragments for this glycan
    /// * `full_formula` the total formula of the whole peptide + glycan
    pub fn generate_theoretical_fragments(
        &self,
        model: &Model,
        peptide_index: usize,
        charge_carriers: &MolecularCharge,
        full_formula: &Multi<MolecularFormula>,
        attachment: (AminoAcid, usize),
    ) -> Vec<Fragment> {
        let single_charges = charge_carriers.all_single_charge_options();
        let all_charges = charge_carriers.all_charge_options();
        model.glycan.as_ref().map_or(vec![], |neutral_losses| {
            // Get all base fragments from this node and all its children
            let mut base_fragments = self
                .oxonium_fragments(peptide_index, attachment)
                .into_iter()
                .flat_map(|f| f.with_charges(&single_charges))
                .flat_map(|f| f.with_neutral_losses(neutral_losses))
                .collect_vec();
            // Generate all Y fragments
            base_fragments.extend(
                self.internal_break_points(attachment)
                    .iter()
                    .filter(|(_, bonds)| {
                        bonds.iter().all(|b| !matches!(b, GlycanBreakPos::B(_)))
                            && !bonds.iter().all(|b| matches!(b, GlycanBreakPos::End(_)))
                    })
                    .flat_map(move |(f, bonds)| {
                        full_formula.iter().map(move |full| {
                            Fragment::new(
                                full - self.formula() + f,
                                Charge::zero(),
                                peptide_index,
                                FragmentType::Y(
                                    bonds
                                        .iter()
                                        .filter(|b| !matches!(b, GlycanBreakPos::End(_)))
                                        .map(GlycanBreakPos::position)
                                        .cloned()
                                        .collect(),
                                ),
                                String::new(),
                            )
                        })
                    })
                    .flat_map(|f| f.with_charges(&all_charges))
                    .flat_map(|f| f.with_neutral_losses(neutral_losses)),
            );
            // Generate all diagnostic ions
            base_fragments.extend(
                self.diagnostic_ions(peptide_index, attachment)
                    .into_iter()
                    .flat_map(|f| f.with_charges(&single_charges)),
            );
            base_fragments
        })
    }

    /// Get uncharged diagnostic ions from all positions
    fn diagnostic_ions(
        &self,
        peptide_index: usize,
        attachment: (AminoAcid, usize),
    ) -> Vec<Fragment> {
        let mut output = self
            .sugar
            .diagnostic_ions(peptide_index, self.position(attachment));
        output.extend(
            self.branches
                .iter()
                .flat_map(|b| b.diagnostic_ions(peptide_index, attachment)),
        );

        output
    }

    /// Generate all fragments without charge and neutral loss options
    fn oxonium_fragments(
        &self,
        peptide_index: usize,
        attachment: (AminoAcid, usize),
    ) -> Vec<Fragment> {
        // Generate the basic single breakage B fragments
        let mut base_fragments = vec![Fragment::new(
            self.formula(),
            Charge::zero(),
            peptide_index,
            FragmentType::B(self.position(attachment)),
            String::new(),
        )];
        // Extend with all internal fragments, meaning multiple breaking bonds
        base_fragments.extend(
            self.internal_break_points(attachment)
                .into_iter()
                .filter(|(_, breakages)| {
                    !breakages
                        .iter()
                        .all(|b| matches!(b, GlycanBreakPos::End(_)))
                })
                .filter(|(m, _)| *m != MolecularFormula::default())
                .map(|(m, b)| {
                    (
                        m,
                        [b, vec![GlycanBreakPos::B(self.position(attachment))]].concat(),
                    )
                })
                .map(|(formula, breakages)| {
                    Fragment::new(
                        formula,
                        Charge::zero(),
                        peptide_index,
                        FragmentType::Oxonium(breakages),
                        String::new(),
                    )
                }),
        );
        // Extend with the theoretical fragments for all branches of this position
        base_fragments.extend(
            self.branches
                .iter()
                .flat_map(|b| b.oxonium_fragments(peptide_index, attachment)),
        );
        base_fragments
    }

    /// All possible bonds that can be broken and the molecular formula that would be held over if these bonds all broke and the broken off parts are lost.
    fn internal_break_points(
        &self,
        attachment: (AminoAcid, usize),
    ) -> Vec<(MolecularFormula, Vec<GlycanBreakPos>)> {
        // Find every internal fragment ending at this bond (in a B breakage) (all bonds found are Y breakages and endings)
        // Walk through all branches and determine all possible breakages
        if self.branches.is_empty() {
            vec![
                (
                    self.formula(),
                    vec![GlycanBreakPos::End(self.position(attachment))],
                ),
                (
                    MolecularFormula::default(),
                    vec![GlycanBreakPos::Y(self.position(attachment))],
                ),
            ]
        } else {
            self.branches
                .iter()
                .map(|b| b.internal_break_points(attachment)) // get all previous options
                .fold(Vec::new(), |accumulator, branch_options| {
                    if accumulator.is_empty() {
                        branch_options
                    } else {
                        let mut new_accumulator = Vec::new();
                        for base in &accumulator {
                            for option in &branch_options {
                                new_accumulator.push((
                                    &option.0 + &base.0,
                                    [option.1.clone(), base.1.clone()].concat(),
                                ));
                            }
                        }
                        new_accumulator
                    }
                })
                .into_iter()
                .map(|(m, b)| (m + self.sugar.formula(), b))
                .chain(std::iter::once((
                    // add the option of it breaking here
                    MolecularFormula::default(),
                    vec![GlycanBreakPos::Y(self.position(attachment))],
                )))
                .collect()
        }
    }

    fn position(&self, attachment: (AminoAcid, usize)) -> GlycanPosition {
        GlycanPosition {
            inner_depth: self.inner_depth,
            series_number: self.outer_depth + 1,
            branch: self.branch.clone(),
            attachment,
        }
    }
}