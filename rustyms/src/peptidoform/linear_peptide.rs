#![warn(dead_code)]

use crate::{
    checked_aminoacid::CheckedAminoAcid,
    fragment::{DiagnosticPosition, Fragment, FragmentType, PeptidePosition},
    glycan::MonoSaccharide,
    helper_functions::{peptide_range_contains, RangeExtension},
    modification::{
        CrossLinkName, GnoComposition, LinkerSpecificity, Modification, SimpleModification,
        SimpleModificationInner,
    },
    molecular_charge::{CachedCharge, MolecularCharge},
    peptidoform::*,
    placement_rule::PlacementRule,
    system::usize::Charge,
    AmbiguousLabel, DiagnosticIon, Element, Model, MolecularFormula, Multi, MultiChemical,
    NeutralLoss, Protease, SequenceElement, SequencePosition,
};
use itertools::Itertools;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{Display, Write},
    marker::PhantomData,
    num::NonZeroU16,
    ops::{Index, IndexMut, RangeBounds},
    slice::SliceIndex,
};

/// A peptide with all data as specified by [ProForma](https://github.com/HUPO-PSI/ProForma).
/// Because the full ProForma specification allows very complex peptides the maximal complexity
/// of a peptide is tracked as a type parameter, This follows the Rust pattern of a
/// [typestate](https://willcrichton.net/rust-api-type-patterns/typestate.html) API.
///
/// The following features are controlled by the complexity parameter:
/// * Cross-links, inter/intra cross-links or branches, only allowed with complexity [`Linked`]
/// * Labile modifications, allowed with complexity [`Linear`] and upwards
/// * Global isotope modifications, allowed with complexity [`Linear`] and upwards
/// * Charge carriers, allowed with complexity [`Linear`] and upwards
/// * Ambiguous modifications, allowed with complexity [`SimpleLinear`] and upwards
/// * Ambiguous amino acid sequence `(?AA)`, allowed with complexity [`SimpleLinear`] and upwards
/// * Ambiguous amino acids (B/Z), allowed with complexity [`SemiAmbiguous`] and upwards
///
/// The following features are always allowed:
/// * N and C terminal modifications (although cross-linkers are only allowed with [`Linked`])
/// * The use of non-standard amino acids that have one chemical formula (J/X/U/O)
/// * [Modification](SimpleModification)s on amino acids
///
/// ## Cross-links
/// Cross-links either bind together two separate peptides or form a loop within a single peptide.
///
/// These can be defined in ProForma by specifying the two positions where the cross-link is bound
/// using the same label:
/// ```text
/// PEC[X:Disulfide#xl1]TIC[#xl1]E
/// PEC[X:Disulfide#xl1]TIDE//OTHERPEC[#xl1]TIDE
/// ```
///
/// ## Labile modifications
/// These modifications are seen in the MS data but are not bound to the precursor. An example
/// could be a glycan in an MS2 experiment that is completely stripped of the precursor in MS2
/// and so cannot be seen on the precursor and cannot be placed on a determinate position.
///
/// These can be defined in ProForma with braces:
/// ```text
/// {Glycan:Hex1HexNac2}PEPTIDE
/// ```
///
/// ## Global isotope modification
/// If a peptide is fully labelled with a specific isotope, only likely to happen for synthetic
/// peptides, this can be defined in ProForma as follows:
/// ```text
/// <15N>PEPTIDE
/// ```
///
/// ## Charge carriers
/// The ions that carry the charge of the peptide can be defined. In ProForma 2.0 the syntax is
/// slightly underspecified but rustyms allows higher charged carriers, e.g. Zn 2+, and complete
/// chemical formulas. If this peptide is part of a [`Peptidoform`] (and so tagged [`Linked`])
/// setting the charge carriers is not allowed on the peptides but
/// [`Peptidoform::set_charge_carriers`] can be used.
/// ```text
/// PEPTIDE/3[1Zn+2,1H+1]
/// ```
///
#[derive(PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Peptidoform<Complexity> {
    /// Global isotope modifications, saved as the element and the species that
    /// all occurrence of that element will consist of. For example (N, 15) will
    /// make all occurring nitrogen atoms be isotope 15.
    global: Vec<(Element, Option<NonZeroU16>)>,
    /// Labile modifications, which will not be found in the actual spectrum.
    labile: Vec<SimpleModification>,
    /// N terminal modifications
    n_term: Vec<Modification>,
    /// C terminal modifications
    c_term: Vec<Modification>,
    /// The sequence of this peptide (includes local modifications)
    sequence: Vec<SequenceElement<Complexity>>,
    /// For each ambiguous modification list all possible positions it can be placed on.
    /// Indexed by the ambiguous modification id.
    modifications_of_unknown_position: Vec<AmbiguousEntry>,
    /// The adduct ions, if specified
    charge_carriers: Option<MolecularCharge>,
    /// The marker indicating which level of complexity this peptide (potentially) uses
    marker: PhantomData<Complexity>,
}

/// An entry in the ambiguous lookup
#[derive(PartialOrd, Ord, Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Hash)]
struct AmbiguousEntry {
    /// The allowed locations for this modification
    positions: Vec<SequencePosition>,
    /// The maximal number of this modification on one place
    limit: Option<usize>,
    /// Determines if this modification can colocalise with other modifications of unknown position
    colocalise_modifications_of_unknown_position: bool,
    /// Group, used for '^x'  
    group: Option<usize>,
}

impl<Complexity> Default for Peptidoform<Complexity> {
    fn default() -> Self {
        Self {
            global: Vec::new(),
            labile: Vec::new(),
            n_term: Vec::new(),
            c_term: Vec::new(),
            sequence: Vec::new(),
            modifications_of_unknown_position: Vec::new(),
            charge_carriers: None,
            marker: PhantomData,
        }
    }
}

impl<Complexity> Clone for Peptidoform<Complexity> {
    fn clone(&self) -> Self {
        Self {
            global: self.global.clone(),
            labile: self.labile.clone(),
            n_term: self.n_term.clone(),
            c_term: self.c_term.clone(),
            sequence: self.sequence.clone(),
            modifications_of_unknown_position: self.modifications_of_unknown_position.clone(),
            charge_carriers: self.charge_carriers.clone(),
            marker: PhantomData,
        }
    }
}

impl<OwnComplexity, OtherComplexity> PartialEq<Peptidoform<OtherComplexity>>
    for Peptidoform<OwnComplexity>
{
    fn eq(&self, other: &Peptidoform<OtherComplexity>) -> bool {
        self.global == other.global
            && self.labile == other.labile
            && self.n_term == other.n_term
            && self.c_term == other.c_term
            && self.sequence == other.sequence
            && self.modifications_of_unknown_position == other.modifications_of_unknown_position
            && self.charge_carriers == other.charge_carriers
    }
}

impl<Complexity> std::hash::Hash for Peptidoform<Complexity> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.global.hash(state);
        self.labile.hash(state);
        self.n_term.hash(state);
        self.c_term.hash(state);
        self.sequence.hash(state);
        self.modifications_of_unknown_position.hash(state);
        self.charge_carriers.hash(state);
    }
}

impl<Complexity> Eq for Peptidoform<Complexity> {}

/// Implement the complexity checks to reduce the complexity of a peptide in a controlled fashion.
impl<Complexity> Peptidoform<Complexity> {
    /// Check if this peptide does not use any of the features reserved for [`Linked`].
    ///
    /// This checks if all modifications (in the sequence and the termini) are [`SimpleModification`]s.
    pub fn is_linear(&self) -> bool {
        self.sequence()
            .iter()
            .all(|seq| seq.modifications.iter().all(|m| !m.is_cross_link()))
            && self.n_term.iter().all(|n| !n.is_cross_link())
            && self.c_term.iter().all(|c| !c.is_cross_link())
    }

    /// Convert this peptide into [`Linear`].
    pub fn into_linear(self) -> Option<Peptidoform<Linear>> {
        if self.is_linear() {
            Some(self.mark())
        } else {
            None
        }
    }

    /// Check if this peptide does not use any of the features reserved for [`Linked`] or [`Linear`].
    ///
    /// This checks if this peptide does not have labile or global modifications and for the absence
    /// of charge carriers.
    pub fn is_simple_linear(&self) -> bool {
        self.is_linear()
            && self.labile.is_empty()
            && self.global.is_empty()
            && self.charge_carriers.is_none()
    }

    /// Convert this peptide into [`SimpleLinear`].
    pub fn into_simple_linear(self) -> Option<Peptidoform<SimpleLinear>> {
        if self.is_simple_linear() {
            Some(self.mark())
        } else {
            None
        }
    }

    /// Check if this peptide does not use any of the features reserved for [`Linked`], [`Linear`],
    /// or [`SimpleLinear`].
    ///
    /// This checks if this peptide does not have any ambiguous modifications or amino acids (`(?AA)` in ProForma).
    pub fn is_semi_ambiguous(&self) -> bool {
        self.is_simple_linear()
            && self.modifications_of_unknown_position.is_empty()
            && !self.sequence.iter().any(|seq| seq.ambiguous.is_some())
    }

    /// Convert this peptide into [`SemiAmbiguous`].
    pub fn into_semi_ambiguous(self) -> Option<Peptidoform<SemiAmbiguous>> {
        if self.is_semi_ambiguous() {
            Some(self.mark())
        } else {
            None
        }
    }

    /// Check if this peptide does not use any of the features reserved for [`Linked`], [`Linear`],
    /// [`SimpleLinear`], or [`SemiAmbiguous`].
    ///
    /// This checks if this peptide does not have B or Z amino acids.
    pub fn is_unambiguous(&self) -> bool {
        self.is_semi_ambiguous()
            && self
                .sequence
                .iter()
                .all(|seq| seq.aminoacid.is_unambiguous())
    }

    /// Convert this peptide into [`UnAmbiguous`].
    pub fn into_unambiguous(self) -> Option<Peptidoform<UnAmbiguous>> {
        if self.is_unambiguous() {
            Some(self.mark())
        } else {
            None
        }
    }
}

impl<Complexity: HighestOf<Linear>> Peptidoform<Complexity> {
    /// Add global isotope modifications, if any is invalid it returns None
    #[must_use]
    pub fn global(
        mut self,
        global: impl IntoIterator<Item = (Element, Option<NonZeroU16>)>,
    ) -> Option<Peptidoform<Complexity::HighestLevel>> {
        for modification in global {
            if modification.0.is_valid(modification.1) {
                self.global.push(modification);
            } else {
                return None;
            }
        }
        Some(self.mark::<Complexity::HighestLevel>())
    }

    /// Add labile modifications
    #[must_use]
    pub fn labile(
        mut self,
        labile: impl IntoIterator<Item = SimpleModification>,
    ) -> Peptidoform<Complexity::HighestLevel> {
        self.labile.extend(labile);
        self.mark::<Complexity::HighestLevel>()
    }
}

impl<Complexity> Peptidoform<Complexity> {
    /// Mark this peptide with the following complexity, be warned that the complexity level is not checked.
    pub(super) fn mark<M>(self) -> Peptidoform<M> {
        Peptidoform {
            global: self.global,
            labile: self.labile,
            n_term: self.n_term,
            c_term: self.c_term,
            sequence: self
                .sequence
                .into_iter()
                .map(SequenceElement::mark)
                .collect(),
            modifications_of_unknown_position: self.modifications_of_unknown_position,
            charge_carriers: self.charge_carriers,
            marker: PhantomData,
        }
    }

    /// Cast a linear peptide into a more complex linear peptide. This undoes any work done by
    /// functions like [`Self::into_linear`]. This does not change the content of the linear
    /// peptide. It only allows to pass this as higher complexity if needed.
    pub fn cast<NewComplexity: AtLeast<Complexity>>(self) -> Peptidoform<NewComplexity> {
        self.mark()
    }

    /// Create a new [`LinearPeptide`], if you want an empty peptide look at [`LinearPeptide::default`].
    /// Potentially the `.collect()` or `.into()` methods can be useful as well.
    #[must_use]
    pub fn new<OtherComplexity: AtMax<Complexity>>(
        sequence: impl IntoIterator<Item = SequenceElement<OtherComplexity>>,
    ) -> Self {
        sequence.into_iter().map(SequenceElement::mark).collect()
    }

    /// Get the sequence for this peptide
    #[must_use]
    pub fn sequence(&self) -> &[SequenceElement<Complexity>] {
        &self.sequence
    }

    /// Get the sequence mutably for the peptide
    #[must_use]
    pub fn sequence_mut(&mut self) -> &mut Vec<SequenceElement<Complexity>> {
        &mut self.sequence
    }

    /// Set the N terminal modifications
    #[must_use]
    pub fn n_term(mut self, term: Vec<Modification>) -> Self {
        self.n_term = term;
        self
    }

    /// Set the C terminal modifications
    #[must_use]
    pub fn c_term(mut self, term: Vec<Modification>) -> Self {
        self.c_term = term;
        self
    }

    /// Set the N terminal modifications
    pub fn set_n_term(&mut self, term: Vec<Modification>) {
        self.n_term = term;
    }

    /// Set the C terminal modifications
    pub fn set_c_term(&mut self, term: Vec<Modification>) {
        self.c_term = term;
    }

    /// Get the number of amino acids making up this peptide
    pub fn len(&self) -> usize {
        self.sequence.len()
    }

    /// Check if the sequence of this peptide is empty (does not contain any amino acids)
    pub fn is_empty(&self) -> bool {
        self.sequence.is_empty()
    }

    /// Get the N terminal modifications.
    pub fn get_n_term(&self) -> &[Modification] {
        &self.n_term
    }

    /// Get the C terminal modifications.
    pub fn get_c_term(&self) -> &[Modification] {
        &self.c_term
    }

    /// Set the N terminal modification as a simple modification
    pub fn add_simple_n_term(&mut self, modification: SimpleModification) {
        self.n_term.push(Modification::Simple(modification));
    }

    /// Set the C terminal modification as a simple modification
    pub fn add_simple_c_term(&mut self, modification: SimpleModification) {
        self.c_term.push(Modification::Simple(modification));
    }

    /// Add a modification to this peptide
    pub fn add_simple_modification(
        &mut self,
        position: SequencePosition,
        modification: SimpleModification,
    ) {
        match position {
            SequencePosition::NTerm => self.add_simple_n_term(modification),
            SequencePosition::CTerm => self.add_simple_c_term(modification),
            SequencePosition::Index(index) => self.sequence[index]
                .modifications
                .push(Modification::Simple(modification)),
        }
    }

    /// Set the charge carriers, use [`Self::charge_carriers`] unless absolutely necessary.
    pub(super) fn set_charge_carriers(&mut self, charge_carriers: Option<MolecularCharge>) {
        self.charge_carriers = charge_carriers;
    }

    /// The mass of the N terminal modifications. The global isotope modifications are NOT applied.
    fn get_n_term_mass(
        &self,
        all_peptides: &[Peptidoform<Linked>],
        visited_peptides: &[usize],
        applied_cross_links: &mut Vec<CrossLinkName>,
        allow_ms_cleavable: bool,
        peptidoform_index: usize,
    ) -> Multi<MolecularFormula> {
        self.n_term.iter().fold(Multi::default(), |acc, f| {
            if let Modification::Ambiguous { .. } = f {
                acc
            } else {
                acc * f
                    .formula_inner(
                        all_peptides,
                        visited_peptides,
                        applied_cross_links,
                        allow_ms_cleavable,
                        SequencePosition::NTerm,
                        peptidoform_index,
                    )
                    .0
            }
        }) + molecular_formula!(H 1)
    }

    /// The mass of the C terminal modifications. The global isotope modifications are NOT applied.
    fn get_c_term_mass(
        &self,
        all_peptides: &[Peptidoform<Linked>],
        visited_peptides: &[usize],
        applied_cross_links: &mut Vec<CrossLinkName>,
        allow_ms_cleavable: bool,
        peptidoform_index: usize,
    ) -> Multi<MolecularFormula> {
        self.c_term.iter().fold(Multi::default(), |acc, f| {
            if let Modification::Ambiguous { .. } = f {
                acc
            } else {
                acc * f
                    .formula_inner(
                        all_peptides,
                        visited_peptides,
                        applied_cross_links,
                        allow_ms_cleavable,
                        SequencePosition::CTerm,
                        peptidoform_index,
                    )
                    .0
            }
        }) + molecular_formula!(H 1 O 1)
    }

    /// Find all neutral losses in the given stretch of peptide (loss, peptide index, sequence index)
    fn potential_neutral_losses(
        &self,
        range: impl RangeBounds<usize>,
        all_peptides: &[Peptidoform<Linked>],
        peptidoform_index: usize,
        ignore_peptides: &mut Vec<usize>,
    ) -> Vec<(NeutralLoss, usize, SequencePosition)> {
        ignore_peptides.push(peptidoform_index);
        let mut found_peptides = Vec::new();
        let own_losses = self
            .iter(range)
            .flat_map(|(pos, aa)| {
                aa.modifications
                    .iter()
                    .filter_map(|modification| match modification {
                        Modification::Simple(modification)
                        | Modification::Ambiguous { modification, .. } => match &**modification {
                            SimpleModificationInner::Database { specificities, .. } => Some(
                                specificities
                                    .iter()
                                    .filter_map(move |(rules, rule_losses, _)| {
                                        if PlacementRule::any_possible(
                                            rules,
                                            aa,
                                            pos.sequence_index,
                                        ) {
                                            Some(rule_losses)
                                        } else {
                                            None
                                        }
                                    })
                                    .flatten()
                                    .map(move |loss| {
                                        (loss.clone(), peptidoform_index, pos.sequence_index)
                                    })
                                    .collect_vec(),
                            ),
                            _ => None, // TODO: potentially hydrolysed cross-linkers could also have neutral losses
                        },
                        Modification::CrossLink {
                            linker,
                            peptide,
                            side,
                            ..
                        } => {
                            if !ignore_peptides.contains(peptide) {
                                found_peptides.push(*peptide);
                            };
                            let (neutral, _, _) = side.allowed_rules(linker);
                            Some(
                                neutral
                                    .into_iter()
                                    .map(|n| (n, peptidoform_index, pos.sequence_index))
                                    .collect_vec(),
                            )
                        }
                    })
                    .flatten()
                    .collect_vec()
            })
            .collect_vec();
        own_losses
            .into_iter()
            .chain(found_peptides.into_iter().flat_map(|p| {
                all_peptides[p].potential_neutral_losses(.., all_peptides, p, ignore_peptides)
            }))
            .collect()
    }

    /// Find all diagnostic ions for this full peptide
    fn diagnostic_ions(&self) -> Vec<(DiagnosticIon, DiagnosticPosition)> {
        self.iter(..)
            .flat_map(|(pos, aa)| {
                aa.diagnostic_ions(pos.sequence_index)
                    .into_iter()
                    .map(move |diagnostic| {
                        (
                            diagnostic,
                            DiagnosticPosition::Peptide(pos, aa.aminoacid.aminoacid()),
                        )
                    })
            })
            .chain(self.labile.iter().flat_map(move |modification| {
                match &**modification {
                    SimpleModificationInner::Database { specificities, .. } => specificities
                        .iter()
                        .flat_map(|(_, _, diagnostic)| diagnostic)
                        .map(|diagnostic| {
                            (
                                diagnostic.clone(),
                                DiagnosticPosition::Labile(modification.clone().into()),
                            )
                        })
                        .collect_vec(),
                    SimpleModificationInner::Linker { specificities, .. } => specificities
                        .iter()
                        .flat_map(|rule| match rule {
                            LinkerSpecificity::Symmetric(_, _, ions)
                            | LinkerSpecificity::Asymmetric(_, _, ions) => ions,
                        })
                        .map(|diagnostic| {
                            (
                                diagnostic.clone(),
                                DiagnosticPosition::Labile(modification.clone().into()),
                            )
                        })
                        .collect_vec(),
                    _ => Vec::new(),
                }
            }))
            .unique()
            .collect()
    }

    /// Iterate over a range in the peptide and keep track of the position
    pub(super) fn iter(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl DoubleEndedIterator<Item = (PeptidePosition, &SequenceElement<Complexity>)> + '_ {
        let start = range.start_index();
        std::iter::once((
            PeptidePosition::n(SequencePosition::NTerm, self.len()),
            &self[SequencePosition::NTerm],
        ))
        .take(usize::from(start == 0))
        .chain(
            self.sequence[(range.start_bound().cloned(), range.end_bound().cloned())]
                .iter()
                .enumerate()
                .map(move |(index, seq)| {
                    (
                        PeptidePosition::n(SequencePosition::Index(index + start), self.len()),
                        seq,
                    )
                }),
        )
        .chain(
            std::iter::once((
                PeptidePosition::n(SequencePosition::CTerm, self.len()),
                &self[SequencePosition::CTerm],
            ))
            .take(usize::from(range.end_index(self.len()) == self.len())),
        )
    }

    /// Generate all possible patterns for the ambiguous positions.
    /// It always contains at least one pattern.
    /// The global isotope modifications are NOT applied.
    /// Additionally it also returns all peptides present as cross-link.
    // TODO: support limit and colocalise
    #[expect(clippy::too_many_arguments)]
    fn ambiguous_patterns(
        &self,
        range: impl RangeBounds<usize>,
        aa_range: impl RangeBounds<usize> + Clone,
        base: &Multi<MolecularFormula>,
        all_peptides: &[Peptidoform<Linked>],
        visited_peptides: &[usize],
        applied_cross_links: &mut Vec<CrossLinkName>,
        allow_ms_cleavable: bool,
        peptidoform_index: usize,
    ) -> (Multi<MolecularFormula>, HashSet<CrossLinkName>) {
        // Calculate all formulas for the selected AA range without any ambiguous modifications
        let (formulas, seen) = self.sequence[(
            aa_range.start_bound().cloned(),
            aa_range.end_bound().cloned(),
        )]
            .iter()
            .enumerate()
            .fold(
                (base.clone(), HashSet::new()),
                |previous_aa_formulas, (index, aa)| {
                    let (f, s) = aa.formulas_base(
                        all_peptides,
                        visited_peptides,
                        applied_cross_links,
                        allow_ms_cleavable,
                        SequencePosition::Index(index),
                        peptidoform_index,
                    );
                    (
                        previous_aa_formulas.0 * f,
                        previous_aa_formulas.1.union(&s).cloned().collect(),
                    )
                },
            );

        // Calculate all masses (and labels) for all possible combinations of ambiguous masses
        let previous_combinations = self
            .modifications_of_unknown_position
            .iter()
            .enumerate()
            .fold(vec![Vec::new()], |previous_combinations, (id, entry)| {
                // Go over all possible locations for this ambiguous mod and add these to all previous options
                let in_range_positions = entry
                    .positions
                    .iter()
                    .filter(|pos| peptide_range_contains(&range, self.len(), **pos))
                    .collect_vec();

                if in_range_positions.is_empty() {
                    // If no location is possible for this modification keep all known combinations
                    previous_combinations
                } else {
                    // Returns a list of all combinations of ambiguous modifications that can go together
                    let mut options = in_range_positions
                        .iter()
                        .flat_map(|pos| {
                            // This position is a possible location, add this location for this mod to all previously known combinations
                            previous_combinations
                                .iter()
                                .filter(|path| {
                                    entry.colocalise_modifications_of_unknown_position
                                        || path.iter().all(|(_, l)| l != pos)
                                })
                                .map(|path| {
                                    let mut new = path.clone();
                                    new.push((id, *pos));
                                    new
                                })
                                .collect_vec()
                        })
                        .collect_vec();
                    // If there is an option to place this mod outside of this range allow that as well
                    // by copying all previous options without any alteration
                    if in_range_positions.len() < entry.positions.len() {
                        options.extend_from_slice(&previous_combinations);
                    }
                    options
                }
            });

        // Determine the formula for all selected ambiguous modifications and create the labels
        let all_ambiguous_options = previous_combinations
            .into_iter()
            .map(|current_selected_ambiguous| {
                current_selected_ambiguous
                    .iter()
                    .copied()
                    .filter_map(|(id, pos)| {
                        match pos {
                            SequencePosition::NTerm => self.n_term.as_slice(),
                            SequencePosition::Index(i) => {
                                self.sequence[*i].modifications.as_slice()
                            }
                            SequencePosition::CTerm => self.c_term.as_slice(),
                        }
                        .iter()
                        .find_map(|m| {
                            if let Modification::Ambiguous {
                                id: mid,
                                modification,
                                ..
                            } = m
                            {
                                (*mid == id).then(|| {
                                    modification
                                        .formula_inner(*pos, peptidoform_index)
                                        .with_label(AmbiguousLabel::Modification {
                                            id,
                                            sequence_index: *pos,
                                            peptidoform_index,
                                        })
                                })
                            } else {
                                None
                            }
                        })
                    })
                    .sum::<MolecularFormula>()
            })
            .collect::<Multi<MolecularFormula>>();
        (formulas * all_ambiguous_options, seen)
    }

    /// Generate the theoretical fragments for this peptide, with the given maximal charge of the fragments, and the given model.
    /// With the global isotope modifications applied.
    /// # Panics
    /// Panics if the `max_charge` is bigger than [`isize::MAX`].
    pub(crate) fn generate_theoretical_fragments_inner(
        &self,
        max_charge: Charge,
        model: &Model,
        peptidoform_ion_index: usize,
        peptidoform_index: usize,
        all_peptides: &[Peptidoform<Linked>],
    ) -> Vec<Fragment> {
        let default_charge = MolecularCharge::proton(
            isize::try_from(max_charge.value)
                .expect("Charge of the precursor cannot be higher then isize::MAX"),
        );
        let mut charge_carriers: CachedCharge = self
            .charge_carriers
            .as_ref()
            .unwrap_or(&default_charge)
            .into();

        let mut output = Vec::with_capacity(20 * self.sequence.len() + 75); // Empirically derived required size of the buffer (Derived from Hecklib)
        for sequence_index in 0..self.sequence.len() {
            let position = PeptidePosition::n(SequencePosition::Index(sequence_index), self.len());
            let mut cross_links = Vec::new();
            let visited_peptides = vec![peptidoform_index];
            let (n_term, n_term_seen) = self.all_masses(
                ..=sequence_index,
                ..sequence_index,
                &self.get_n_term_mass(
                    all_peptides,
                    &visited_peptides,
                    &mut cross_links,
                    model.allow_cross_link_cleavage,
                    peptidoform_index,
                ),
                model.modification_specific_neutral_losses,
                all_peptides,
                &visited_peptides,
                &mut cross_links,
                model.allow_cross_link_cleavage,
                peptidoform_index,
            );
            let (c_term, c_term_seen) = self.all_masses(
                sequence_index..,
                sequence_index + 1..,
                &self.get_c_term_mass(
                    all_peptides,
                    &visited_peptides,
                    &mut cross_links,
                    model.allow_cross_link_cleavage,
                    peptidoform_index,
                ),
                model.modification_specific_neutral_losses,
                all_peptides,
                &visited_peptides,
                &mut cross_links,
                model.allow_cross_link_cleavage,
                peptidoform_index,
            );
            if !n_term_seen.is_disjoint(&c_term_seen) {
                continue; // There is a link reachable from both sides so there is a loop
            }
            let (modifications_total, modifications_cross_links) = self.sequence[sequence_index]
                .modifications
                .iter()
                .fold((Multi::default(), HashSet::new()), |acc, m| {
                    let (f, s) = m.formula_inner(
                        all_peptides,
                        &[peptidoform_index],
                        &mut cross_links,
                        model.allow_cross_link_cleavage,
                        SequencePosition::Index(sequence_index),
                        peptidoform_index,
                    );
                    (acc.0 * f, acc.1.union(&s).cloned().collect())
                });

            output.append(
                &mut self.sequence[sequence_index]
                    .aminoacid
                    .aminoacid()
                    .fragments(
                        &n_term,
                        &c_term,
                        &modifications_total,
                        &mut charge_carriers,
                        SequencePosition::Index(sequence_index),
                        self.sequence.len(),
                        &model.ions(position),
                        peptidoform_ion_index,
                        peptidoform_index,
                        (
                            // Allow any N terminal fragment if there is no cross-link to the C terminal side
                            c_term_seen.is_disjoint(&modifications_cross_links),
                            n_term_seen.is_disjoint(&modifications_cross_links),
                        ),
                    ),
            );

            if model.m {
                //  p - sX fragment: precursor amino acid side chain losses
                output.extend(
                    self.formulas_inner(
                        peptidoform_index,
                        all_peptides,
                        &[],
                        &mut Vec::new(),
                        model.allow_cross_link_cleavage,
                    )
                    .0
                    .iter()
                    .flat_map(|m| {
                        self.sequence[sequence_index]
                            .aminoacid
                            .formulas_inner(
                                SequencePosition::Index(sequence_index),
                                peptidoform_index,
                            )
                            .iter()
                            .flat_map(|aa| {
                                Fragment::generate_all(
                                    &((-modifications_total.clone()) + m.clone() - aa.clone()
                                        + molecular_formula!(C 2 H 2 N 1 O 1)),
                                    peptidoform_ion_index,
                                    peptidoform_index,
                                    &FragmentType::PrecursorSideChainLoss(
                                        position,
                                        self.sequence[sequence_index].aminoacid.aminoacid(),
                                    ),
                                    &Multi::default(),
                                    &[],
                                    &mut charge_carriers,
                                    model.precursor.1,
                                )
                            })
                            .collect_vec()
                    }),
                );
            }
        }
        for fragment in &mut output {
            fragment.formula = fragment.formula.as_ref().map(|f| {
                f.with_global_isotope_modifications(&self.global)
                    .expect("Invalid global isotope modification")
            });
        }

        // Generate precursor peak
        let (full_precursor, _all_cross_links) = self.formulas_inner(
            peptidoform_index,
            all_peptides,
            &[],
            &mut Vec::new(),
            model.allow_cross_link_cleavage,
        );
        // Allow neutral losses from modifications for the precursor
        let mut precursor_neutral_losses = if model.modification_specific_neutral_losses {
            self.potential_neutral_losses(.., all_peptides, peptidoform_index, &mut Vec::new())
                .into_iter()
                .map(|(n, _, _)| n)
                .collect_vec()
        } else {
            Vec::new()
        };
        precursor_neutral_losses.extend_from_slice(&model.precursor.0);

        output.extend(Fragment::generate_all(
            &full_precursor,
            peptidoform_ion_index,
            peptidoform_index,
            &FragmentType::Precursor,
            &Multi::default(),
            &precursor_neutral_losses,
            &mut charge_carriers,
            model.precursor.1,
        ));

        // Add glycan fragmentation to all peptide fragments
        // Assuming that only one glycan can ever fragment at the same time,
        // and that no peptide fragmentation occurs during glycan fragmentation
        let full_formula = self
            .formulas_inner(
                peptidoform_index,
                all_peptides,
                &[],
                &mut Vec::new(),
                model.allow_cross_link_cleavage,
            )
            .0;
        for (sequence_index, position) in self.sequence.iter().enumerate() {
            let attachment = (position.aminoacid.aminoacid(), sequence_index);
            for modification in &position.modifications {
                output.extend(modification.generate_theoretical_fragments(
                    model,
                    peptidoform_ion_index,
                    peptidoform_index,
                    &mut charge_carriers,
                    &full_formula,
                    Some(attachment),
                ));
            }
        }

        if model.modification_specific_diagnostic_ions.0 {
            // Add all modification diagnostic ions
            for (dia, pos) in self.diagnostic_ions() {
                output.extend(
                    Fragment {
                        formula: Some(dia.0),
                        charge: Charge::default(),
                        ion: FragmentType::Diagnostic(pos),
                        peptidoform_ion_index: Some(peptidoform_ion_index),
                        peptidoform_index: Some(peptidoform_index),
                        neutral_loss: Vec::new(),
                        deviation: None,
                        confidence: None,
                        auxiliary: false,
                    }
                    .with_charge_range(
                        &mut charge_carriers,
                        model.modification_specific_diagnostic_ions.1,
                    ),
                );
            }
        }

        // Add labile glycan fragments
        for modification in &self.labile {
            match &**modification {
                SimpleModificationInner::Glycan(composition) => {
                    output.extend(MonoSaccharide::theoretical_fragments(
                        composition,
                        model,
                        peptidoform_ion_index,
                        peptidoform_index,
                        &mut charge_carriers,
                        &full_formula,
                        None,
                    ));
                }
                SimpleModificationInner::GlycanStructure(structure)
                | SimpleModificationInner::Gno {
                    composition: GnoComposition::Topology(structure),
                    ..
                } => {
                    output.extend(
                        structure
                            .clone()
                            .determine_positions()
                            .generate_theoretical_fragments(
                                model,
                                peptidoform_ion_index,
                                peptidoform_index,
                                &mut charge_carriers,
                                &full_formula,
                                None,
                            ),
                    );
                }
                _ => (),
            }
        }

        output
    }

    /// Generate all potential masses for the given stretch of amino acids alongside all peptides seen as part of a cross-link.
    /// Applies ambiguous amino acids and modifications, and neutral losses (if allowed in the model).
    // TODO: take terminal ambiguous into account
    #[expect(clippy::too_many_arguments)]
    fn all_masses(
        &self,
        range: impl RangeBounds<usize> + Clone,
        aa_range: impl RangeBounds<usize> + Clone,
        base: &Multi<MolecularFormula>,
        apply_neutral_losses: bool,
        all_peptides: &[Peptidoform<Linked>],
        visited_peptides: &[usize],
        applied_cross_links: &mut Vec<CrossLinkName>,
        allow_ms_cleavable: bool,
        peptidoform_index: usize,
    ) -> (Multi<MolecularFormula>, HashSet<CrossLinkName>) {
        let (ambiguous_mods_masses, seen) = self.ambiguous_patterns(
            range.clone(),
            aa_range,
            base,
            all_peptides,
            visited_peptides,
            applied_cross_links,
            allow_ms_cleavable,
            peptidoform_index,
        );
        if apply_neutral_losses {
            let neutral_losses = self.potential_neutral_losses(
                range,
                all_peptides,
                peptidoform_index,
                &mut Vec::new(),
            );
            let mut all_masses =
                Vec::with_capacity(ambiguous_mods_masses.len() * (1 + neutral_losses.len()));
            all_masses.extend(ambiguous_mods_masses.iter().cloned());
            for loss in &neutral_losses {
                all_masses.extend((ambiguous_mods_masses.clone() + loss.0.clone()).to_vec());
            }
            (all_masses.into(), seen)
        } else {
            (ambiguous_mods_masses, seen)
        }
    }

    /// Get the total amount of ambiguous modifications
    pub(crate) fn number_of_ambiguous_modifications(&self) -> usize {
        self.modifications_of_unknown_position.len()
    }

    /// Gives all the formulas for the whole peptide with no C and N terminal modifications. With the global isotope modifications applied.
    #[expect(clippy::missing_panics_doc)] // Global isotope mods are guaranteed to be correct
    fn bare_formulas_inner(
        &self,
        all_peptides: &[Peptidoform<Linked>],
        visited_peptides: &[usize],
        applied_cross_links: &mut Vec<CrossLinkName>,
        allow_ms_cleavable: bool,
        peptidoform_index: usize,
    ) -> Multi<MolecularFormula> {
        let mut formulas = Multi::default();
        let mut placed = vec![false; self.modifications_of_unknown_position.len()];
        for (index, pos) in self.sequence.iter().enumerate() {
            formulas *= pos
                .formulas_greedy(
                    &mut placed,
                    all_peptides,
                    visited_peptides,
                    applied_cross_links,
                    allow_ms_cleavable,
                    SequencePosition::Index(index),
                    peptidoform_index,
                )
                .0;
        }

        formulas
            .iter()
            .map(|f| {
                f.with_global_isotope_modifications(&self.global)
                    .expect("Invalid global isotope modification in bare_formulas")
            })
            .collect()
    }

    /// Gives the formulas for the whole peptide. With the global isotope modifications applied. (Any B/Z will result in multiple possible formulas.)
    /// # Panics
    /// When this peptide is already in the set of visited peptides.
    pub(crate) fn formulas_inner(
        &self,
        peptidoform_index: usize,
        all_peptides: &[Peptidoform<Linked>],
        visited_peptides: &[usize],
        applied_cross_links: &mut Vec<CrossLinkName>,
        allow_ms_cleavable: bool,
    ) -> (Multi<MolecularFormula>, HashSet<CrossLinkName>) {
        debug_assert!(
            !visited_peptides.contains(&peptidoform_index),
            "Cannot get the formula for a peptide that is already visited"
        );
        let mut new_visited_peptides = vec![peptidoform_index];
        new_visited_peptides.extend_from_slice(visited_peptides);
        let mut formulas: Multi<MolecularFormula> = self.get_n_term_mass(
            all_peptides,
            visited_peptides,
            applied_cross_links,
            allow_ms_cleavable,
            peptidoform_index,
        ) * self.get_c_term_mass(
            all_peptides,
            visited_peptides,
            applied_cross_links,
            allow_ms_cleavable,
            peptidoform_index,
        );
        let mut placed = vec![false; self.modifications_of_unknown_position.len()];
        let mut seen = HashSet::new();
        for (index, pos) in self.sequence.iter().enumerate() {
            let (pos_f, pos_seen) = pos.formulas_greedy(
                &mut placed,
                all_peptides,
                &new_visited_peptides,
                applied_cross_links,
                allow_ms_cleavable,
                SequencePosition::Index(index),
                peptidoform_index,
            );
            formulas *= pos_f;
            seen.extend(pos_seen);
        }

        (formulas
            .iter()
            .map(|f| f.with_global_isotope_modifications(&self.global).expect("Global isotope modification invalid in determination of all formulas for a peptide"))
            .collect(), seen)
    }

    /// Display this peptide.
    /// `specification_compliant` Displays this peptide either normalised to the internal
    /// representation or as fully spec compliant ProForma (no glycan structure or custom modifications).
    /// # Errors
    /// If the formatter supplied errors.
    /// # Panics
    /// If there is an ambiguous modification without a definition, this indicates an error in rustyms.
    pub fn display(
        &self,
        f: &mut impl Write,
        show_global_mods: bool,
        specification_compliant: bool,
    ) -> std::fmt::Result {
        if show_global_mods {
            for (element, isotope) in &self.global {
                write!(
                    f,
                    "<{}{}>",
                    isotope.map(|i| i.to_string()).unwrap_or_default(),
                    element
                )?;
            }
        }
        for labile in &self.labile {
            write!(f, "{{{labile}}}")?;
        }
        // Write any modification of unknown position that has no preferred location at the start of the peptide
        let mut any_ambiguous = false;
        let mut placed_ambiguous = Vec::new();
        let mut preferred_ambiguous_position =
            vec![None; self.modifications_of_unknown_position.len()];
        for (id, ambiguous) in self.modifications_of_unknown_position.iter().enumerate() {
            if let Some(preferred) = ambiguous
                .positions
                .iter()
                .find_map(|i| {
                    let m = match i {
                        SequencePosition::NTerm => self.n_term.iter().find(|m| {
                            if let Modification::Ambiguous { id: mid, .. } = m {
                                *mid == id
                            } else {
                                false
                            }
                        }),
                        SequencePosition::Index(i) => {
                            self.sequence[*i].modifications.iter().find(|m| {
                                if let Modification::Ambiguous { id: mid, .. } = m {
                                    *mid == id
                                } else {
                                    false
                                }
                            })
                        }
                        SequencePosition::CTerm => self.c_term.iter().find(|m| {
                            if let Modification::Ambiguous { id: mid, .. } = m {
                                *mid == id
                            } else {
                                false
                            }
                        }),
                    };

                    if let Some(Modification::Ambiguous {
                        id: mid, preferred, ..
                    }) = m
                    {
                        (*mid == id && *preferred).then_some(*i)
                    } else {
                        None
                    }
                })
                .or_else(|| (ambiguous.positions.len() == 1).then_some(ambiguous.positions[0]))
            {
                preferred_ambiguous_position[id] = Some(preferred);
            } else {
                let m = match ambiguous.positions.first() {
                    Some(SequencePosition::NTerm) => self.n_term.iter().find(|m| {
                        if let Modification::Ambiguous { id: mid, .. } = m {
                            *mid == id
                        } else {
                            false
                        }
                    }),
                    Some(SequencePosition::Index(i)) => {
                        self.sequence[*i].modifications.iter().find(|m| {
                            if let Modification::Ambiguous { id: mid, .. } = m {
                                *mid == id
                            } else {
                                false
                            }
                        })
                    }
                    Some(SequencePosition::CTerm) => self.c_term.iter().find(|m| {
                        if let Modification::Ambiguous { id: mid, .. } = m {
                            *mid == id
                        } else {
                            false
                        }
                    }),
                    None => None,
                };
                if let Some(m) = m {
                    write!(f, "[")?;
                    m.display(f, specification_compliant, true)?;
                    write!(f, "]")?;
                    placed_ambiguous.push(id);
                    any_ambiguous = true;
                }
            }
        }
        if any_ambiguous {
            write!(f, "?")?;
        }
        let mut any_n = false;
        for m in self.get_n_term() {
            let mut display_ambiguous = false;

            if let Modification::Ambiguous { id, .. } = m {
                if !placed_ambiguous.contains(id) && preferred_ambiguous_position[*id].is_none()
                    || preferred_ambiguous_position[*id]
                        .is_some_and(|p| p == SequencePosition::NTerm)
                {
                    display_ambiguous = true;
                    placed_ambiguous.push(*id);
                }
            }

            write!(f, "[")?;
            m.display(f, specification_compliant, display_ambiguous)?;
            write!(f, "]")?;
            any_n = true;
        }
        if any_n {
            write!(f, "-")?;
        }
        let mut last_ambiguous = None;
        for (index, position) in self.sequence.iter().enumerate() {
            placed_ambiguous.extend(position.display(
                f,
                &placed_ambiguous,
                &preferred_ambiguous_position,
                index,
                last_ambiguous,
                specification_compliant,
            )?);
            last_ambiguous = position.ambiguous;
        }
        if last_ambiguous.is_some() {
            write!(f, ")")?;
        }
        let mut first = true;
        for m in self.get_c_term() {
            let mut display_ambiguous = false;
            if let Modification::Ambiguous { id, .. } = m {
                display_ambiguous = !placed_ambiguous.contains(id);
                placed_ambiguous.push(*id);
            }
            if first {
                write!(f, "-")?;
                first = false;
            }
            write!(f, "[")?;
            m.display(f, specification_compliant, display_ambiguous)?;
            write!(f, "]")?;
        }
        if let Some(c) = &self.charge_carriers {
            write!(f, "/{c}")?;
        }
        Ok(())
    }

    /// Get the reverse of this peptide
    #[must_use]
    pub fn reverse(&self) -> Self {
        Self {
            n_term: self.c_term.clone(),
            c_term: self.n_term.clone(),
            sequence: self.sequence.clone().into_iter().rev().collect(),
            modifications_of_unknown_position: self
                .modifications_of_unknown_position
                .clone()
                .into_iter()
                .map(|m| AmbiguousEntry {
                    positions: m
                        .positions
                        .into_iter()
                        .map(|loc| loc.reverse(self.len()))
                        .collect(),
                    ..m
                })
                .collect(),
            ..self.clone()
        }
    }
    /// Get all labile modifications
    pub(super) fn get_labile_mut_inner(&mut self) -> &mut Vec<SimpleModification> {
        &mut self.labile
    }
}

impl Peptidoform<Linked> {
    /// Add a modification to this peptide
    pub(crate) fn add_modification(
        &mut self,
        position: SequencePosition,
        modification: Modification,
    ) {
        match position {
            SequencePosition::NTerm => self.n_term.push(modification),
            SequencePosition::CTerm => self.c_term.push(modification),
            SequencePosition::Index(index) => self.sequence[index].modifications.push(modification),
        }
    }

    /// Set the N terminal modification
    pub fn add_n_term(&mut self, modification: Modification) {
        self.n_term.push(modification);
    }

    /// Set the C terminal modification
    pub fn add_c_term(&mut self, modification: Modification) {
        self.c_term.push(modification);
    }
}

impl Peptidoform<Linear> {
    /// Add the charge carriers.
    #[must_use]
    pub fn charge_carriers(mut self, charge: Option<MolecularCharge>) -> Self {
        self.charge_carriers = charge;
        self
    }
}

impl<Complexity: AtMax<Linear>> Peptidoform<Complexity> {
    /// Get a region of this peptide as a new peptide (with all terminal/global/ambiguous modifications).
    #[must_use]
    pub fn sub_peptide(&self, index: impl RangeBounds<usize>) -> Self {
        Self {
            n_term: if index.contains(&0) {
                self.n_term.clone()
            } else {
                Vec::new()
            },
            c_term: if index.contains(&(self.len() - 1)) {
                self.c_term.clone()
            } else {
                Vec::new()
            },
            sequence: self.sequence[(index.start_bound().cloned(), index.end_bound().cloned())]
                .to_vec(),
            ..self.clone()
        }
    }

    /// Digest this sequence with the given protease and the given maximal number of missed cleavages.
    pub fn digest(&self, protease: &Protease, max_missed_cleavages: usize) -> Vec<Self> {
        let mut sites = vec![0];
        sites.extend_from_slice(&protease.match_locations(&self.sequence));
        sites.push(self.len());

        let mut result = Vec::new();

        for (index, start) in sites.iter().enumerate() {
            for end in sites.iter().skip(index).take(max_missed_cleavages + 1) {
                result.push(self.sub_peptide((*start)..*end));
            }
        }
        result
    }

    /// Get the N terminal modifications as simple modifications
    pub fn get_simple_n_term(&self) -> Vec<SimpleModification> {
        self.n_term
            .iter()
            .filter_map(|m| m.clone().into_simple())
            .collect()
    }

    /// Get the C terminal modifications as simple modifications
    pub fn get_simple_c_term(&self) -> Vec<SimpleModification> {
        self.c_term
            .iter()
            .filter_map(|m| m.clone().into_simple())
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
    ) -> Vec<Fragment> {
        self.generate_theoretical_fragments_inner(max_charge, model, 0, 0, &[])
    }

    /// Gives the formulas for the whole peptide. With the global isotope modifications applied. (Any B/Z will result in multiple possible formulas.)
    #[expect(clippy::missing_panics_doc)] // Can not panic (unless state is already corrupted)
    pub fn formulas(&self) -> Multi<MolecularFormula> {
        let mut formulas: Multi<MolecularFormula> =
            self.get_n_term_mass(&[], &[], &mut Vec::new(), false, 0)
                * self.get_c_term_mass(&[], &[], &mut Vec::new(), false, 0);
        let mut placed = vec![false; self.modifications_of_unknown_position.len()];
        for (index, pos) in self.sequence.iter().enumerate() {
            formulas *= pos
                .formulas_greedy(
                    &mut placed,
                    &[],
                    &[],
                    &mut Vec::new(),
                    false,
                    SequencePosition::Index(index),
                    0,
                )
                .0;
        }

        formulas
            .iter()
            .map(|f| f.with_global_isotope_modifications(&self.global).expect("Global isotope modification invalid in determination of all formulas for a peptide"))
            .collect()
    }

    /// Gives all the formulas for the whole peptide with no C and N terminal modifications. With the global isotope modifications applied.
    pub fn bare_formulas(&self) -> Multi<MolecularFormula> {
        self.bare_formulas_inner(&[], &[], &mut Vec::new(), false, 0)
    }
}

impl Peptidoform<UnAmbiguous> {
    /// Gives the formula for the whole peptide. With the global isotope modifications applied.
    #[expect(clippy::missing_panics_doc)] // Can not panic (unless state is already corrupted)
    pub fn formula(&self) -> MolecularFormula {
        let mut options = self
            .formulas_inner(0, &[], &[], &mut Vec::new(), false)
            .0
            .to_vec();
        assert_eq!(options.len(), 1);
        options.pop().unwrap()
    }

    /// Gives the formula for the whole peptide with no C and N terminal modifications. With the global isotope modifications applied.
    #[expect(clippy::missing_panics_doc)] // Can not panic (unless state is already corrupted)
    pub fn bare_formula(&self) -> MolecularFormula {
        let mut options = self
            .bare_formulas_inner(&[], &[], &mut Vec::new(), false, 0)
            .to_vec();
        assert_eq!(options.len(), 1);
        options.pop().unwrap()
    }
}

impl<Complexity: AtLeast<Linear>> Peptidoform<Complexity> {
    /// Get the global isotope modifications
    pub fn get_global(&self) -> &[(Element, Option<NonZeroU16>)] {
        &self.global
    }

    /// Get the global isotope modifications
    pub fn get_global_mut(&mut self) -> &mut Vec<(Element, Option<NonZeroU16>)> {
        &mut self.global
    }

    /// Add the global isotope modification, if any is invalid it returns false
    #[must_use]
    pub fn add_global(&mut self, modification: (Element, Option<NonZeroU16>)) -> bool {
        if modification.0.is_valid(modification.1) {
            self.global.push(modification);
            true
        } else {
            false
        }
    }

    /// Get all labile modifications
    pub fn get_labile(&self) -> &[SimpleModification] {
        &self.labile
    }

    /// Get all labile modifications
    pub fn get_labile_mut(&mut self) -> &mut Vec<SimpleModification> {
        &mut self.labile
    }

    /// Get the charge carriers, if there are any
    pub const fn get_charge_carriers(&self) -> Option<&MolecularCharge> {
        self.charge_carriers.as_ref()
    }

    /// Get the charge carriers, if there are any
    pub fn get_charge_carriers_mut(&mut self) -> Option<&mut MolecularCharge> {
        self.charge_carriers.as_mut()
    }
}

impl<Complexity: AtLeast<SimpleLinear>> Peptidoform<Complexity> {
    /// Get the locations of all ambiguous modifications. The slice is indexed by ambiguous
    /// modification id and contains all sequence locations where that ambiguous modification is
    /// potentially located.
    pub fn get_ambiguous_modifications(&self) -> Vec<Vec<SequencePosition>> {
        self.modifications_of_unknown_position
            .iter()
            .map(|e| e.positions.clone())
            .collect()
    }

    /// Add a new global modification of unknown position. If the modification would be placed on a
    /// terminal but something is already placed there it is ignored.
    /// # Errors
    /// When there are no possible locations return false, the modification is then not applied.
    #[must_use]
    pub fn add_unknown_position_modification(
        &mut self,
        modification: SimpleModification,
        range: impl RangeBounds<usize>,
        settings: &crate::MUPSettings,
    ) -> bool {
        let possible_positions = self
            .iter(range)
            .filter(|(position, seq)| {
                modification
                    .is_possible(seq, position.sequence_index)
                    .any_possible()
                    && (settings.position.is_none()
                        || settings.position.as_ref().is_some_and(|rules| {
                            rules
                                .iter()
                                .any(|rule| rule.is_possible(seq, position.sequence_index))
                        }))
                    && (settings.colocalise_placed_modifications
                        || self[position.sequence_index]
                            .modifications
                            .iter()
                            .all(Modification::is_ambiguous))
            })
            .map(|(position, _)| (position.sequence_index, None))
            .collect_vec();

        self.add_ambiguous_modification(
            modification,
            None,
            &possible_positions,
            None,
            settings.limit,
            settings.colocalise_modifications_of_unknown_position,
        )
    }

    /// Add an ambiguous modification on the given positions, the placement rules are NOT checked.
    /// The `positions` contains all sequence indices where that ambiguous modification is
    /// potentially located alongside the placement probability if known. If there is a preferred
    /// position this can be indicated as well. If the modification would be placed on a terminal
    /// but something is already placed there it is ignored.
    /// # Errors
    /// When there are no possible locations return false, the modification is then not applied.
    #[must_use]
    pub fn add_ambiguous_modification(
        &mut self,
        modification: SimpleModification,
        group: Option<String>,
        positions: &[(SequencePosition, Option<OrderedFloat<f64>>)],
        preferred_position: Option<SequencePosition>,
        limit: Option<usize>,
        colocalise_modifications_of_unknown_position: bool,
    ) -> bool {
        match positions.len() {
            0 => false,
            1 => {
                match positions[0].0 {
                    SequencePosition::NTerm => {
                        self.n_term.push(modification.into());
                    }
                    SequencePosition::Index(pos) => {
                        self.sequence[pos].modifications.push(modification.into());
                        self.sequence[pos].modifications.sort_unstable();
                    }
                    SequencePosition::CTerm => {
                        self.c_term.push(modification.into());
                    }
                }
                true
            }
            _ => {
                let id = self.modifications_of_unknown_position.len();
                let group = group.unwrap_or_else(|| format!("u{id}"));
                let mut placed = false;
                self.modifications_of_unknown_position.push(AmbiguousEntry {
                    positions: positions
                        .iter()
                        .map(|(spos, score)| match spos {
                            SequencePosition::NTerm => {
                                self.n_term.push(Modification::Ambiguous {
                                    id,
                                    group: group.clone(),
                                    modification: modification.clone(),
                                    localisation_score: *score,
                                    preferred: preferred_position.is_some_and(|p| p == *spos),
                                });
                                placed = true;
                                *spos
                            }
                            SequencePosition::Index(pos) => {
                                self.sequence[*pos]
                                    .modifications
                                    .push(Modification::Ambiguous {
                                        id,
                                        group: group.clone(),
                                        modification: modification.clone(),
                                        localisation_score: *score,
                                        preferred: preferred_position.is_some_and(|p| p == *spos),
                                    });
                                self.sequence[*pos].modifications.sort_unstable();
                                placed = true;
                                *spos
                            }
                            SequencePosition::CTerm => {
                                self.c_term.push(Modification::Ambiguous {
                                    id,
                                    group: group.clone(),
                                    modification: modification.clone(),
                                    localisation_score: *score,
                                    preferred: preferred_position.is_some_and(|p| p == *spos),
                                });
                                placed = true;
                                *spos
                            }
                        })
                        .collect(),
                    limit,
                    colocalise_modifications_of_unknown_position,
                    group: None,
                });
                placed
            }
        }
    }
}

impl<OwnComplexity: AtMax<SemiAmbiguous>> Peptidoform<OwnComplexity> {
    /// Concatenate another peptide after this peptide. This will fail if any of these conditions are true:
    /// * This peptide has a C terminal modification
    /// * The other peptide has an N terminal modification
    // Because it is complexity SemiAmbiguous these peptides are guaranteed to not contain charge
    // carriers, global or ambiguous modifications.
    pub fn concatenate<OtherComplexity: AtMax<SemiAmbiguous>>(
        self,
        other: Peptidoform<OtherComplexity>,
    ) -> Option<Peptidoform<OwnComplexity::HighestLevel>>
    where
        OwnComplexity: HighestOf<OtherComplexity>,
    {
        if self.c_term.is_empty() && other.n_term.is_empty() {
            Some(Peptidoform::<OwnComplexity::HighestLevel> {
                global: self.global,
                labile: self.labile.into_iter().chain(other.labile).collect(),
                n_term: self.n_term,
                c_term: other.c_term,
                sequence: self
                    .sequence
                    .into_iter()
                    .map(SequenceElement::mark)
                    .chain(other.sequence.into_iter().map(SequenceElement::mark))
                    .collect(),
                modifications_of_unknown_position: Vec::new(),
                charge_carriers: self.charge_carriers,
                marker: PhantomData,
            })
        } else {
            None
        }
    }
}

impl<Complexity> Display for Peptidoform<Complexity> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.display(f, true, true)
    }
}

impl<Collection, Item, Complexity> From<Collection> for Peptidoform<Complexity>
where
    Collection: IntoIterator<Item = Item>,
    Item: Into<SequenceElement<Complexity>>,
{
    fn from(value: Collection) -> Self {
        Self {
            global: Vec::new(),
            labile: Vec::new(),
            n_term: Vec::new(),
            c_term: Vec::new(),
            sequence: value.into_iter().map(std::convert::Into::into).collect(),
            modifications_of_unknown_position: Vec::new(),
            charge_carriers: None,
            marker: PhantomData,
        }
    }
}

impl<Item, Complexity> FromIterator<Item> for Peptidoform<Complexity>
where
    Item: Into<SequenceElement<Complexity>>,
{
    fn from_iter<Iter: IntoIterator<Item = Item>>(iter: Iter) -> Self {
        Self::from(iter)
    }
}

impl<I: SliceIndex<[SequenceElement<Complexity>]>, Complexity> Index<I>
    for Peptidoform<Complexity>
{
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.sequence[index]
    }
}

impl<Complexity> Index<SequencePosition> for Peptidoform<Complexity> {
    type Output = SequenceElement<Complexity>;

    fn index(&self, index: SequencePosition) -> &Self::Output {
        match index {
            SequencePosition::NTerm => &self.sequence[0],
            SequencePosition::Index(i) => &self.sequence[i],
            SequencePosition::CTerm => self.sequence.last().unwrap(),
        }
    }
}

impl<Complexity> IndexMut<SequencePosition> for Peptidoform<Complexity> {
    fn index_mut(&mut self, index: SequencePosition) -> &mut Self::Output {
        match index {
            SequencePosition::NTerm => &mut self.sequence[0],
            SequencePosition::Index(i) => &mut self.sequence[i],
            SequencePosition::CTerm => self.sequence.last_mut().unwrap(),
        }
    }
}

/// Make sure that any lower level of Peptidoform can be cast to a higher level
macro_rules! into {
    ($a:tt => $b:ty) => {
        impl From<Peptidoform<$a>> for Peptidoform<$b> {
            fn from(other: Peptidoform<$a>) -> Self {
                other.mark()
            }
        }
        impl From<SequenceElement<$a>> for SequenceElement<$b> {
            fn from(other: SequenceElement<$a>) -> Self {
                other.mark()
            }
        }
        impl From<CheckedAminoAcid<$a>> for CheckedAminoAcid<$b> {
            fn from(other: CheckedAminoAcid<$a>) -> Self {
                other.mark()
            }
        }
    };
}

into!(Linear => Linked);
into!(SimpleLinear => Linked);
into!(SemiAmbiguous => Linked);
into!(UnAmbiguous => Linked);
into!(SimpleLinear => Linear);
into!(SemiAmbiguous => Linear);
into!(UnAmbiguous => Linear);
into!(SemiAmbiguous => SimpleLinear);
into!(UnAmbiguous => SimpleLinear);
into!(UnAmbiguous => SemiAmbiguous);
