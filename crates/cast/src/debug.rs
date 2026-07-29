use alloy_chains::Chain;
use alloy_primitives::{Bytes, map::AddressHashMap};
use foundry_cli::utils::{TraceResult, print_traces};
use foundry_common::{ContractsByArtifact, compile::ProjectCompiler};
use foundry_config::{Config, TracingConfig};
use foundry_debugger::Debugger;
use foundry_evm::{
    hardforks::TempoHardfork,
    traces::{
        CallTraceDecoder, CallTraceDecoderBuilder, DebugTraceIdentifier, Traces,
        debug::ContractSources,
        decode_trace_arena,
        identifier::{SignaturesIdentifier, TraceIdentifiers},
    },
};
use foundry_evm_networks::{NetworkExecutionContext, ResolvedNetworkProfile};

async fn decode_debugger_traces(traces: &mut Traces, decoder: &CallTraceDecoder) {
    for (_, trace) in traces {
        decode_trace_arena(trace, decoder).await;
    }
}

/// labels the traces, conditionally prints them or opens the debugger
#[expect(clippy::too_many_arguments)]
pub(crate) async fn handle_traces(
    mut result: TraceResult,
    config: &Config,
    chain: Chain,
    contracts_bytecode: &AddressHashMap<Bytes>,
    tracing: &TracingConfig,
    with_local_artifacts: bool,
    debug: bool,
    tempo_hardfork: Option<TempoHardfork>,
    network_profile: ResolvedNetworkProfile,
    network_context: NetworkExecutionContext,
) -> eyre::Result<()> {
    let (known_contracts, mut sources) = if with_local_artifacts {
        // Status prose goes to stderr so `--json` output on stdout stays machine-readable.
        let _ = sh_status!("Compiling project to generate artifacts");
        let project = config.project()?;
        let compiler = ProjectCompiler::new();
        let output = compiler.compile(&project)?;
        (
            Some(ContractsByArtifact::new(
                output.artifact_ids().map(|(id, artifact)| (id, artifact.clone().into())),
            )),
            ContractSources::from_project_output(&output, project.root(), None)?,
        )
    } else {
        (None, ContractSources::default())
    };

    let is_tempo = tempo_hardfork.is_some() || chain.is_tempo();
    let mut builder = CallTraceDecoderBuilder::new()
        .with_tracing_config(tracing)
        .with_network_profile(network_profile, network_context)
        .with_signature_identifier(SignaturesIdentifier::from_config(config)?)
        .with_chain_id((!is_tempo).then(|| chain.id()))
        .with_tempo_hardfork(
            tempo_hardfork
                .or_else(|| chain.is_tempo().then(|| config.evm_spec_id::<TempoHardfork>())),
        );
    let mut identifier = TraceIdentifiers::new().with_external(config, Some(chain))?;
    if let Some(contracts) = &known_contracts {
        builder = builder.with_known_contracts(contracts);
        identifier = identifier.with_local_and_bytecodes(contracts, contracts_bytecode);
    }

    let mut decoder = builder.build();

    for (_, trace) in result.traces.as_deref_mut().unwrap_or_default() {
        decoder.identify(trace, &mut identifier);
    }

    if tracing.decode_internal || debug {
        if let Some(ref etherscan_identifier) = identifier.external {
            sources.merge(etherscan_identifier.get_compiled_contracts().await?);
        }

        if debug {
            if let Some(traces) = result.traces.as_mut() {
                decode_debugger_traces(traces, &decoder).await;
            }
            let mut debugger = Debugger::builder()
                .traces(result.traces.expect("missing traces"))
                .decoder(&decoder)
                .sources(sources)
                .build();
            debugger.try_run_tui()?;
            return Ok(());
        }

        decoder.debug_identifier = Some(DebugTraceIdentifier::new(sources));
    }

    print_traces(
        &mut result,
        &decoder,
        tracing.verbosity > 0,
        tracing.verbosity > 4,
        tracing.trace_depth,
    )
    .await?;

    Ok(())
}

#[cfg(all(test, feature = "hashkey"))]
mod tests {
    use super::*;
    use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
    use alloy_json_abi::Function;
    use alloy_primitives::{Address, U256};
    use foundry_evm::traces::{
        CallTrace, CallTraceArena, CallTraceNode, SparsedTraceArena, TraceKind,
    };
    use foundry_evm_networks::{NetworkConfigs, NetworkTraceIdentity};

    #[tokio::test]
    async fn hashkey_debugger_projection_decodes_b20_calls() {
        let mut token = [0u8; 20];
        token[0] = 0xb2;
        token[11..].fill(0x11);
        let token = Address::from(token);
        let recipient = Address::repeat_byte(0x22);
        let mint = Function::parse("mint(address to,uint256 amount)").unwrap();
        let mut arena = CallTraceArena::default();
        arena.nodes_mut()[0] = CallTraceNode {
            trace: CallTrace {
                address: token,
                data: mint
                    .abi_encode_input(&[
                        DynSolValue::Address(recipient),
                        DynSolValue::Uint(U256::from(42), 256),
                    ])
                    .unwrap()
                    .into(),
                success: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut traces = vec![(
            TraceKind::Execution,
            SparsedTraceArena {
                arena,
                ignored: Default::default(),
                diagnostics: Default::default(),
            },
        )];
        let profile = NetworkConfigs::with_hashkey().resolve();
        let context = NetworkExecutionContext::new(177, 0);
        assert_eq!(profile.trace_identity(token, context), Some(NetworkTraceIdentity::B20Asset));
        let decoder = CallTraceDecoderBuilder::new().with_network_profile(profile, context).build();

        decode_debugger_traces(&mut traces, &decoder).await;

        let decoded = traces[0].1.nodes()[0].trace.decoded.as_deref().unwrap();
        assert_eq!(decoded.label.as_deref(), Some("B20Asset"));
        let call = decoded.call_data.as_ref().unwrap();
        assert_eq!(call.signature, "mint(address,uint256)");
        assert_eq!(call.args, [recipient.to_string(), "42".to_string()]);
    }
}
