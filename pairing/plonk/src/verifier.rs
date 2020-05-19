/********************************************************************************************

This source file implements zk-proof batch verifier functionality.

*********************************************************************************************/

use rand_core::RngCore;
use crate::index::{VerifierIndex as Index};
use oracle::rndoracle::{ProofError};
pub use super::prover::{ProverProof, RandomOracles};
use algebra::{Field, PrimeField, PairingEngine, ProjectiveCurve, VariableBaseMSM};
use oracle::sponge::FqSponge;
use crate::plonk_sponge::FrSponge;
use ff_fft::Evaluations;

impl<E: PairingEngine> ProverProof<E>
{
    // This function verifies the batch of zk-proofs
    //     proofs: vector of Plonk proofs
    //     index: Index
    //     rng: randomness source context
    //     RETURN: verification status
    pub fn verify
        <EFqSponge: FqSponge<E::Fq, E::G1Affine, E::Fr>,
         EFrSponge: FrSponge<E::Fr>,
        >
    (
        proofs: &Vec<ProverProof<E>>,
        index: &Index<E>,
        rng: &mut dyn RngCore
    ) -> Result<bool, ProofError>
    {
        let mut batch = Vec::new();
        for proof in proofs.iter()
        {
            let proof = proof.clone();
            let oracles = proof.oracles::<EFqSponge, EFrSponge>(index)?;
            let zeta2 = oracles.zeta.pow(&[index.domain.size]);
            let zeta3 = zeta2.pow(&[index.domain.size]);

            let t_comm = VariableBaseMSM::multi_scalar_mul
            (
                &[proof.tlow_comm, proof.tmid_comm, proof.thgh_comm],
                &[E::Fr::one().into_repr(), zeta2.into_repr(), zeta3.into_repr()]
            ).into_affine();

            let ab = (proof.evals.a + &(oracles.beta * &proof.evals.sigma1) + &oracles.gamma) *
                &(proof.evals.b + &(oracles.beta * &proof.evals.sigma2) + &oracles.gamma) * &oracles.alpha;

            let t =
                (proof.evals.r +
                &Evaluations::<E::Fr>::from_vec_and_domain(proof.public.clone(), index.domain).interpolate().evaluate(oracles.zeta) -
                &(ab * &(proof.evals.c + &oracles.gamma) * &proof.evals.z) -
                &index.l1.evaluate(oracles.zeta)) / &(zeta2 - &E::Fr::one());

            let r_comm = VariableBaseMSM::multi_scalar_mul
            (
                &[index.qm_comm, index.ql_comm, index.qr_comm, index.qo_comm, index.qc_comm, proof.z_comm, index.sigma_comm[2]],
                &[
                    (proof.evals.a * &proof.evals.b).into_repr(), proof.evals.a.into_repr(),
                    proof.evals.b.into_repr(), proof.evals.c.into_repr(), E::Fr::one().into_repr(),
                    (
                        ((proof.evals.a + &(oracles.beta * &oracles.zeta) + &oracles.gamma) *
                        &(proof.evals.b + &(oracles.beta * &index.r * &oracles.zeta) + &oracles.gamma) *
                        &(proof.evals.c + &(oracles.beta * &index.o * &oracles.zeta) + &oracles.gamma) * &proof.evals.z) * &oracles.alpha +
                        &(index.l1.evaluate(oracles.zeta) * &oracles.alpha.square())
                    ).into_repr(),
                    (ab * &oracles.beta * &proof.evals.z).into_repr(),
                ]
            ).into_affine();
    
            batch.push
            ((
                oracles.zeta,
                oracles.v,
                vec!
                [
                    (t_comm,                t, None),
                    (r_comm,                proof.evals.r, None),
                    (proof.a_comm,          proof.evals.a, None),
                    (proof.b_comm,          proof.evals.b, None),
                    (proof.c_comm,          proof.evals.c, None),
                    (index.sigma_comm[0],   proof.evals.sigma1, None),
                    (index.sigma_comm[1],   proof.evals.sigma2, None),
                ],
                proof.proof1
            ));
            batch.push
            ((
                oracles.zeta * &index.domain.group_gen,
                oracles.v,
                vec![(proof.z_comm, proof.evals.z, None)],
                proof.proof2
            ));
        }
        match index.urs.verify(&batch, rng)
        {
            false => Err(ProofError::OpenProof),
            true => Ok(true)
        }
    }

    // This function queries random oracle values from non-interactive
    // argument context by verifier
    pub fn oracles
        <EFqSponge: FqSponge<E::Fq, E::G1Affine, E::Fr>,
         EFrSponge: FrSponge<E::Fr>,
        >
    (
        &self,
        index: &Index<E>
    ) -> Result<RandomOracles<E::Fr>, ProofError>
    {
        let mut oracles = RandomOracles::<E::Fr>::zero();
        let mut fq_sponge = EFqSponge::new(index.fq_sponge_params.clone());

        // absorb the public input, a, b, c polycommitments into the argument
        fq_sponge.absorb_fr(&self.public);
        fq_sponge.absorb_g(&[self.a_comm, self.b_comm, self.c_comm]);
        // sample beta, gamma oracles
        oracles.beta = fq_sponge.challenge();
        oracles.gamma = fq_sponge.challenge();

        // absorb the z commitment into the argument and query alpha
        fq_sponge.absorb_g(&[self.z_comm]);
        oracles.alpha = fq_sponge.challenge();

        // absorb the polycommitments into the argument and sample zeta
        fq_sponge.absorb_g(&[self.tlow_comm, self.tmid_comm, self.thgh_comm]);
        oracles.zeta = fq_sponge.challenge();
        // query opening scaler challenge
        oracles.v = fq_sponge.challenge();

        Ok(oracles)
    }
}
