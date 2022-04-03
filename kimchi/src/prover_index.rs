//! This module implements the prover index as [ProverIndex].

use crate::alphas::Alphas;
use crate::circuits::domain_constant_evaluation::DomainConstantEvaluations;
use crate::circuits::{
    constraints::ConstraintSystem,
    expr::{Linearization, PolishToken},
    wires::*,
};
use crate::linearization::expr_linearization;
use ark_ec::AffineCurve;
use ark_ff::PrimeField;
use commitment_dlog::{commitment::CommitmentCurve, srs::SRS};
use oracle::poseidon::ArithmeticSpongeParams;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::sync::Arc;

type Fr<G> = <G as AffineCurve>::ScalarField;
type Fq<G> = <G as AffineCurve>::BaseField;

/// The index used by the prover
#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
//~spec:startcode
pub struct ProverIndex<G: CommitmentCurve> {

    /// constraints system polynomials
    #[serde(skip)]
    pub cs: ConstraintSystem<Fr<G>, DomainConstantEvaluations<Fr<G>>>,

    /// The symbolic linearization of our circuit, which can compile to concrete types once certain values are learned in the protocol.
    #[serde(skip)]
    pub linearization: Linearization<Vec<PolishToken<Fr<G>>>>,

    /// The mapping between powers of alpha and constraints
    #[serde(skip)]
    pub powers_of_alpha: Alphas<Fr<G>>,

    /// polynomial commitment keys
    #[serde(skip)]
    pub srs: Arc<SRS<G>>,

    /// maximal size of polynomial section
    pub max_poly_size: usize,

    /// maximal size of the quotient polynomial according to the supported constraints
    pub max_quot_size: usize,

    /// random oracle argument parameters
    #[serde(skip)]
    pub fq_sponge_params: ArithmeticSpongeParams<Fq<G>>,
}
//~spec:endcode

impl<'a, G: CommitmentCurve> ProverIndex<G>
where
    G::BaseField: PrimeField,
{
    /// this function compiles the index from constraints
    pub fn create(
        mut cs: ConstraintSystem<Fr<G>, DomainConstantEvaluations<Fr<G>>>,
        fq_sponge_params: ArithmeticSpongeParams<Fq<G>>,
        endo_q: Fr<G>,
        srs: Arc<SRS<G>>,
    ) -> Self {
        let max_poly_size = srs.g.len();
        if cs.public > 0 {
            assert!(
                max_poly_size >= cs.domain.d1.size as usize,
                "polynomial segment size has to be not smaller that that of the circuit!"
            );
        }
        cs.endo = endo_q;

        // pre-compute the linearization
        let (linearization, powers_of_alpha) = expr_linearization(
            cs.domain.d1,
            cs.chacha8.is_some(),
            &cs.lookup_constraint_system,
        );

        // set `max_quot_size` to the degree of the quotient polynomial,
        // which is obtained by looking at the highest monomial in the sum
        // $$\sum_{i=0}^{PERMUTS} (w_i(x) + \beta k_i x + \gamma)$$
        // where the $w_i(x)$ are of degree the size of the domain.
        let max_quot_size = PERMUTS * cs.domain.d1.size as usize;

        ProverIndex {
            cs,
            linearization,
            powers_of_alpha,
            srs,
            max_poly_size,
            max_quot_size,
            fq_sponge_params,
        }
    }
}

pub mod testing {
    use super::*;
    use crate::circuits::gate::CircuitGate;
    use commitment_dlog::srs::endos;
    use mina_curves::pasta::{pallas::Affine as Other, vesta::Affine, Fp};

    pub fn new_index_for_test(gates: Vec<CircuitGate<Fp>>, public: usize) -> ProverIndex<Affine> {
        let fp_sponge_params = oracle::pasta::fp_kimchi::params();
        let cs = ConstraintSystem::<Fp, DomainConstantEvaluations<Fp>>::create(
            gates,
            vec![],
            fp_sponge_params,
            public,
        )
        .unwrap();

        let mut srs = SRS::<Affine>::create(cs.domain.d1.size as usize);
        srs.add_lagrange_basis(cs.domain.d1);
        let srs = Arc::new(srs);

        let fq_sponge_params = oracle::pasta::fq_kimchi::params();
        let (endo_q, _endo_r) = endos::<Other>();
        ProverIndex::<Affine>::create(cs, fq_sponge_params, endo_q, srs)
    }
}
