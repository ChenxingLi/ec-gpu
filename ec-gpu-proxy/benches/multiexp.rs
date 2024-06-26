use std::sync::Arc;

use ag_build::generate;
use ag_types::PrimeFieldRepr;
use ark_bls12_381::{Fr as Scalar, G1Affine};
use ark_std::UniformRand;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};
use ec_gpu_proxy::{
    multiexp::MultiexpKernel, multiexp_cpu::SourceBuilder, threadpool::Worker,
};
use rust_gpu_tools::Device;

use rayon::iter::{IntoParallelIterator, ParallelIterator};

/// The power that will be used to define the maximum number of elements. The
/// number of elements is `2^MAX_ELEMENTS_POWER`.
const MAX_ELEMENTS_POWER: usize = 24;
/// The maximum number of elements for this benchmark.
const MAX_ELEMENTS: usize = 1 << MAX_ELEMENTS_POWER;

fn build_multiexp() {
    generate(&ag_build::SourceBuilder::new().add_multiexp::<G1Affine>())
}

fn bench_multiexp(crit: &mut Criterion) {
    let mut group = crit.benchmark_group("multiexp");
    // The difference between runs is so little, hence a low sample size is OK.
    group.sample_size(10);

    build_multiexp();
    let devices = Device::all();
    let programs = devices
        .iter()
        .map(|device| ec_gpu_program::load_program!(device))
        .collect::<Result<_, _>>()
        .expect("Cannot create programs!");
    let mut kern = MultiexpKernel::<G1Affine>::create(programs, &devices)
        .expect("Cannot initialize kernel!");
    let pool = Worker::new();
    let max_bases: Vec<_> = (0..MAX_ELEMENTS)
        .into_par_iter()
        .map(|_| G1Affine::rand(&mut rand::thread_rng()))
        .collect();
    let max_exponents: Vec<_> = (0..MAX_ELEMENTS)
        .into_par_iter()
        .map(|_| Scalar::rand(&mut rand::thread_rng()).to_repr())
        .collect();

    let num_elements: Vec<_> =
        (16..MAX_ELEMENTS_POWER).map(|shift| 1 << shift).collect();
    for num in num_elements {
        group.bench_with_input(
            BenchmarkId::from_parameter(num),
            &num,
            |bencher, &num| {
                let (bases, skip) = SourceBuilder::get((
                    Arc::new(max_bases[0..num].to_vec()),
                    0,
                ));
                let exponents = Arc::new(max_exponents[0..num].to_vec());

                bencher.iter(|| {
                    let _ = black_box(
                        kern.multiexp(
                            &pool,
                            bases.clone(),
                            exponents.clone(),
                            skip,
                        )
                        .unwrap(),
                    );
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_multiexp);
criterion_main!(benches);
