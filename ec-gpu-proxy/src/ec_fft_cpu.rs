use ag_types::GpuCurveAffine;
use ark_ff::{Field, PrimeField, Zero};
use std::ops::MulAssign;

use crate::{pow_vartime, threadpool::Worker};

/// Calculate the Fast Fourier Transform on the CPU (single-threaded).
///
/// The input `a` is mutated and contains the result when this function returns.
/// The length of the input vector must be `2^log_n`.
#[allow(clippy::many_single_char_names)]
pub fn serial_ec_fft<G: GpuCurveAffine>(
    a: &mut [G::Curve], omega: &G::Scalar, log_n: u32,
) where G::Scalar: PrimeField {
    fn bitreverse(mut n: u32, l: u32) -> u32 {
        let mut r = 0;
        for _ in 0..l {
            r = (r << 1) | (n & 1);
            n >>= 1;
        }
        r
    }

    let n = a.len() as u32;
    assert_eq!(n, 1 << log_n);

    for k in 0..n {
        let rk = bitreverse(k, log_n);
        if k < rk {
            a.swap(rk as usize, k as usize);
        }
    }

    let mut m = 1;
    for _ in 0..log_n {
        let w_m = pow_vartime(omega, &[u64::from(n / (2 * m))]);

        let mut k = 0;
        while k < n {
            let mut w = G::Scalar::ONE;
            for j in 0..m {
                let mut t = a[(k + j + m) as usize];
                t.mul_assign(w);
                let mut tmp = a[(k + j) as usize];
                tmp -= t;
                a[(k + j + m) as usize] = tmp;
                a[(k + j) as usize] += t;
                w *= w_m;
            }

            k += 2 * m;
        }

        m *= 2;
    }
}

/// Calculate the Fast Fourier Transform on the CPU (multithreaded).
///
/// The result is is written to the input `a`.
/// The number of threads used will be `2^log_threads`.
/// There must be more items to process than threads.
pub fn parallel_ec_fft<G: GpuCurveAffine>(
    a: &mut [G::Curve], worker: &Worker, omega: &G::Scalar, log_n: u32,
    log_threads: u32,
) where
    G::Scalar: PrimeField,
{
    assert!(log_n >= log_threads);

    let num_threads = 1 << log_threads;
    let log_new_n = log_n - log_threads;
    let mut tmp = vec![vec![G::Curve::zero(); 1 << log_new_n]; num_threads];
    let new_omega = pow_vartime(omega, &[num_threads as u64]);

    worker.scope(0, |scope, _| {
        let a = &*a;

        for (j, tmp) in tmp.iter_mut().enumerate() {
            scope.execute(move || {
                // Shuffle into a sub-FFT
                let omega_j = pow_vartime(omega, &[j as u64]);
                let omega_step = pow_vartime(omega, &[(j as u64) << log_new_n]);

                let mut elt = G::Scalar::ONE;
                for (i, tmp) in tmp.iter_mut().enumerate() {
                    for s in 0..num_threads {
                        let idx = (i + (s << log_new_n)) % (1 << log_n);
                        let mut t = a[idx];
                        t *= elt;
                        *tmp += t;
                        elt *= omega_step;
                    }
                    elt *= omega_j;
                }

                // Perform sub-FFT
                serial_ec_fft::<G>(tmp, &new_omega, log_new_n);
            });
        }
    });

    // TODO: does this hurt or help?
    worker.scope(a.len(), |scope, chunk| {
        let tmp = &tmp;

        for (idx, a) in a.chunks_mut(chunk).enumerate() {
            scope.execute(move || {
                let mut idx = idx * chunk;
                let mask = (1 << log_threads) - 1;
                for a in a {
                    *a = tmp[idx & mask][idx >> log_threads];
                    idx += 1;
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use ark_ff::FftField;

    fn omega<F: FftField>(num_coeffs: usize) -> F {
        // Compute omega, the 2^exp primitive root of unity
        let exp = (num_coeffs as f32).log2().floor() as u32;
        let mut omega = F::TWO_ADIC_ROOT_OF_UNITY;
        for _ in exp..F::TWO_ADICITY {
            omega = omega.square();
        }
        omega
    }

    #[test]
    fn parallel_ec_fft_consistency() {
        use super::*;

        use chosen_ark_suite::G1Affine;
        use rand_core::RngCore;
        use std::cmp::min;

        fn test_consistency<G: GpuCurveAffine, R: RngCore>(rng: &mut R)
        where G::Scalar: PrimeField {
            let worker = Worker::new();

            for _ in 0..5 {
                for log_d in 0..10 {
                    let d = 1 << log_d;

                    let mut v1_coeffs = (0..d)
                        .map(|_| G::rand(&mut *rng).into_group())
                        .collect::<Vec<_>>();
                    let mut v2_coeffs = v1_coeffs.clone();
                    let v1_omega = omega::<G::Scalar>(v1_coeffs.len());
                    let v2_omega = v1_omega;

                    for log_threads in log_d..min(log_d + 1, 3) {
                        parallel_ec_fft::<G>(
                            &mut v1_coeffs,
                            &worker,
                            &v1_omega,
                            log_d,
                            log_threads,
                        );
                        serial_ec_fft::<G>(&mut v2_coeffs, &v2_omega, log_d);

                        assert!(v1_coeffs == v2_coeffs);
                    }
                }
            }
        }

        let rng = &mut rand::thread_rng();

        test_consistency::<G1Affine, _>(rng);
    }
}
