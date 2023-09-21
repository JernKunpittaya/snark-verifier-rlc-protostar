use crate::{
    cost::{Cost, CostEstimation},
    loader::{LoadedScalar, Loader},
    pcs::{
        kzg::{KzgAccumulator, KzgAs, KzgSuccinctVerifyingKey},
        PolynomialCommitmentScheme, Query,
    },
    util::{
        arithmetic::{CurveAffine, MultiMillerLoop, Rotation},
        msm::Msm,
        transcript::TranscriptRead,
        Itertools,
    },
    Error,
};

/// Verifier of multi-open KZG. It is for the GWC implementation
/// in [`halo2_proofs`].
/// Notations are following <https://eprint.iacr.org/2019/953.pdf>.
#[derive(Clone, Debug)]
pub struct Gwc19;

impl<M, L> PolynomialCommitmentScheme<M::G1Affine, L> for KzgAs<M, Gwc19>
where
    M: MultiMillerLoop,
    M::G1Affine: CurveAffine<ScalarExt = M::Fr, CurveExt = M::G1>,
    L: Loader<M::G1Affine>,
{
    type VerifyingKey = KzgSuccinctVerifyingKey<M::G1Affine>;
    type Proof = Gwc19Proof<M::G1Affine, L>;
    type Output = KzgAccumulator<M::G1Affine, L>;

    fn read_proof<T>(
        _: &Self::VerifyingKey,
        queries: &[Query<Rotation>],
        transcript: &mut T,
    ) -> Result<Self::Proof, Error>
    where
        T: TranscriptRead<M::G1Affine, L>,
    {
        Gwc19Proof::read(queries, transcript)
    }

    fn verify(
        svk: &Self::VerifyingKey,
        commitments: &[Msm<M::G1Affine, L>],
        z: &L::LoadedScalar,
        queries: &[Query<Rotation, L::LoadedScalar>],
        proof: &Self::Proof,
    ) -> Result<Self::Output, Error> {
        let sets = query_sets(queries);
        let powers_of_u = &proof.u.powers(sets.len());
        let f = {
            let powers_of_v = proof.v.powers(sets.iter().map(|set| set.polys.len()).max().unwrap());
            sets.iter()
                .map(|set| set.msm(commitments, &powers_of_v))
                .zip(powers_of_u.iter())
                .map(|(msm, power_of_u)| msm * power_of_u)
                .sum::<Msm<_, _>>()
        };
        let z_omegas = sets.iter().map(|set| {
            let loaded_shift = set.loaded_shift.clone();
            loaded_shift * z
        });

        let rhs = proof
            .ws
            .iter()
            .zip(powers_of_u.iter())
            .map(|(w, power_of_u)| Msm::base(w) * power_of_u)
            .collect_vec();
        let lhs = f + rhs.iter().zip(z_omegas).map(|(uw, z_omega)| uw.clone() * &z_omega).sum();

        Ok(KzgAccumulator::new(
            lhs.evaluate(Some(svk.g)),
            rhs.into_iter().sum::<Msm<_, _>>().evaluate(Some(svk.g)),
        ))
    }
}

/// Structured proof of [`Gwc19`].
#[derive(Clone, Debug)]
pub struct Gwc19Proof<C, L>
where
    C: CurveAffine,
    L: Loader<C>,
{
    v: L::LoadedScalar,
    ws: Vec<L::LoadedEcPoint>,
    u: L::LoadedScalar,
}

impl<C, L> Gwc19Proof<C, L>
where
    C: CurveAffine,
    L: Loader<C>,
{
    fn read<T>(queries: &[Query<Rotation>], transcript: &mut T) -> Result<Self, Error>
    where
        T: TranscriptRead<C, L>,
    {
        let v = transcript.squeeze_challenge();
        let ws = transcript.read_n_ec_points(query_sets(queries).len())?;
        let u = transcript.squeeze_challenge();
        Ok(Gwc19Proof { v, ws, u })
    }
}

struct QuerySet<'a, S, T> {
    shift: S,
    loaded_shift: T,
    polys: Vec<usize>,
    evals: Vec<&'a T>,
}

impl<'a, S, T> QuerySet<'a, S, T>
where
    T: Clone,
{
    fn msm<C: CurveAffine, L: Loader<C, LoadedScalar = T>>(
        &self,
        commitments: &[Msm<'a, C, L>],
        powers_of_v: &[L::LoadedScalar],
    ) -> Msm<C, L>
    where
        T: LoadedScalar<C::Scalar>,
    {
        self.polys
            .iter()
            .zip(self.evals.iter().cloned())
            .map(|(poly, eval)| {
                let commitment = commitments[*poly].clone();
                commitment - Msm::constant(eval.clone())
            })
            .zip(powers_of_v.iter())
            .map(|(msm, power_of_v)| msm * power_of_v)
            .sum()
    }
}

fn query_sets<S, T>(queries: &[Query<S, T>]) -> Vec<QuerySet<S, T>>
where
    S: PartialEq + Copy,
    T: Clone + PartialEq,
{
    queries.iter().fold(Vec::new(), |mut sets, query| {
        if let Some(pos) = sets.iter().position(|set| set.shift == query.shift) {
            sets[pos].polys.push(query.poly);
            sets[pos].evals.push(&query.eval);
        } else {
            sets.push(QuerySet {
                shift: query.shift,
                loaded_shift: query.loaded_shift.clone(),
                polys: vec![query.poly],
                evals: vec![&query.eval],
            });
        }
        sets
    })
}

impl<M> CostEstimation<M::G1Affine> for KzgAs<M, Gwc19>
where
    M: MultiMillerLoop,
{
    type Input = Vec<Query<Rotation>>;

    fn estimate_cost(queries: &Vec<Query<Rotation>>) -> Cost {
        let num_w = query_sets(queries).len();
        Cost { num_commitment: num_w, num_msm: num_w, ..Default::default() }
    }
}
