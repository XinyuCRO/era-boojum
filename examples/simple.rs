use boojum::algebraic_props::round_function::AbsorptionModeOverwrite;
use boojum::algebraic_props::sponge::GoldilocksPoseidonSponge;
use boojum::config::DevCSConfig;
use boojum::cs::cs_builder::{new_builder, CsBuilder, CsBuilderImpl};
use boojum::cs::cs_builder_reference::CsReferenceImplementationBuilder;
use boojum::cs::cs_builder_verifier::CsVerifierBuilder;
use boojum::cs::gates::{
    ConstantAllocatableCS, ConstantsAllocatorGate, FmaGateInBaseFieldWithoutConstant, NopGate,
    PublicInputGate,
};
use boojum::cs::implementations::{
    pow::NoPow, prover::ProofConfig, transcript::GoldilocksPoisedonTranscript,
};
use boojum::cs::traits::{cs::ConstraintSystem, gate::GatePlacementStrategy};
use boojum::cs::{CSGeometry, GateConfigurationHolder, Place, StaticToolboxHolder};
use boojum::field::goldilocks::{GoldilocksExt2, GoldilocksField};
use boojum::field::U64Representable;
use boojum::gadgets::num::Num;
use boojum::worker::Worker;

fn main() {
    type P = GoldilocksField;
    type F = GoldilocksField;

    let geometry = CSGeometry {
        num_columns_under_copy_permutation: 8,
        num_witness_columns: 1,
        num_constant_columns: 2,
        max_allowed_constraint_degree: 8,
    };

    let max_variables = 512;
    let max_trace_len = 128;

    fn configure<
        T: CsBuilderImpl<F, T>,
        GC: GateConfigurationHolder<F>,
        TB: StaticToolboxHolder,
    >(
        builder: CsBuilder<T, F, GC, TB>,
    ) -> CsBuilder<T, F, impl GateConfigurationHolder<F>, impl StaticToolboxHolder> {
        let builder = ConstantsAllocatorGate::configure_builder(
            builder,
            GatePlacementStrategy::UseGeneralPurposeColumns,
        );
        let builder = FmaGateInBaseFieldWithoutConstant::configure_builder(
            builder,
            GatePlacementStrategy::UseGeneralPurposeColumns,
        );
        let builder =
            NopGate::configure_builder(builder, GatePlacementStrategy::UseGeneralPurposeColumns);

        let builder = PublicInputGate::configure_builder(
            builder,
            GatePlacementStrategy::UseGeneralPurposeColumns,
        );

        builder
    }

    let builder_impl = CsReferenceImplementationBuilder::<F, P, DevCSConfig>::new(
        geometry,
        max_variables,
        max_trace_len,
    );
    let builder = new_builder::<_, F>(builder_impl);

    let builder = configure(builder);
    let mut cs = builder.build(());

    // I know: x * x - 4x + 7, when x = 1, is 4
    // a = x * x
    // b = 4 * x
    // c = a + b
    // result = c + 7

    let x = cs.alloc_single_variable_from_witness(GoldilocksField::from_u64_unchecked(1));
    let public_input_gate = PublicInputGate::new(x.into());
    public_input_gate.add_to_cs(&mut cs);

    let x = Num::<F>::from_variable(x);

    let four = Num::from_variable(cs.allocate_constant(F::from_u64_unchecked(4)));
    let seven = Num::from_variable(cs.allocate_constant(F::from_u64_unchecked(7)));

    let a = x.mul(&mut cs, &x);
    let b = x.mul(&mut cs, &four);
    let c = a.sub(&mut cs, &b);
    let result = c.add(&mut cs, &seven);

    let result = result.get_variable();
    let public_input_gate = PublicInputGate::new(result);
    public_input_gate.add_to_cs(&mut cs);

    let result = cs.get_value(Place::from_variable(result)).wait().unwrap()[0];

    println!("result = {}", result);

    // optional
    cs.pad_and_shrink();

    let worker = Worker::new_with_num_threads(1);

    let cs = cs.into_assembly();

    let lde_factor_to_use = 32;
    let mut proof_config = ProofConfig::default();
    proof_config.fri_lde_factor = lde_factor_to_use;
    proof_config.pow_bits = 0;

    let (proof, vk) = cs.prove_one_shot::<
        GoldilocksExt2,
        GoldilocksPoisedonTranscript,
        GoldilocksPoseidonSponge<AbsorptionModeOverwrite>,
        NoPow,
    >(&worker, proof_config, ());

    proof
        .public_inputs
        .iter()
        .for_each(|x| println!("public_inputs = {}", x));

    // println!("proof: {:?}", proof);
    // println!("vk: {:?}", vk);

    let builder_impl = CsVerifierBuilder::<F, GoldilocksExt2>::new_from_parameters(geometry);
    let builder = new_builder::<_, F>(builder_impl);

    let builder = configure(builder);
    let verifier = builder.build(());

    let is_valid = verifier.verify::<
        GoldilocksPoseidonSponge<AbsorptionModeOverwrite>,
        GoldilocksPoisedonTranscript,
        NoPow
    >(
        (),
        &vk,
        &proof,
    );

    println!("is_valid = {}", is_valid);
}
