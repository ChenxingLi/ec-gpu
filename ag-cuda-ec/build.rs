fn main() {
    use ag_build::{generate, SourceBuilder};

    let source = SourceBuilder::new().add_ec_fft::<ark_bn254::G1Affine>();
    generate(&source);
}
