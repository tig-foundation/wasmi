use super::{error::TestError, TestContext, TestDescriptor};
use anyhow::Result;
use wasmi::Config;
use wasmi_core::{Value, F32, F64};
use wast::{
    core::{NanPattern, WastRetCore},
    lexer::Lexer,
    parser::ParseBuffer,
    token::Span,
    QuoteWat,
    Wast,
    WastDirective,
    WastExecute,
    WastInvoke,
    WastRet,
    Wat,
};

/// Runs the Wasm test spec identified by the given name.
pub fn run_wasm_spec_test(name: &str, config: Config) {
    let test = TestDescriptor::new(name);
    let mut context = TestContext::new(&test, config);

    let mut lexer = Lexer::new(test.file());
    lexer.allow_confusing_unicode(true);
    let parse_buffer = match ParseBuffer::new_with_lexer(lexer) {
        Ok(buffer) => buffer,
        Err(error) => panic!(
            "failed to create ParseBuffer for {}: {}",
            test.path(),
            error
        ),
    };
    let wast = match wast::parser::parse(&parse_buffer) {
        Ok(wast) => wast,
        Err(error) => panic!(
            "failed to parse `.wast` spec test file for {}: {}",
            test.path(),
            error
        ),
    };

    execute_directives(wast, &mut context).unwrap_or_else(|error| {
        panic!(
            "{}: failed to execute `.wast` directive: {}",
            test.path(),
            error
        )
    });

    println!("profiles: {:#?}", context.profile());
}

fn execute_directives(wast: Wast, test_context: &mut TestContext) -> Result<()> {
    'outer: for directive in wast.directives {
        test_context.profile().bump_directives();
        match directive {
            WastDirective::Wat(QuoteWat::Wat(Wat::Module(module))) => {
                test_context.compile_and_instantiate(module)?;
                test_context.profile().bump_module();
            }
            WastDirective::Wat(_) => {
                test_context.profile().bump_quote_module();
                // For the purpose of testing `wasmi` we are not
                // interested in parsing `.wat` files, therefore
                // we silently ignore this case for now.
                // This might change once wasmi supports `.wat` files.
                continue 'outer;
            }
            WastDirective::AssertMalformed {
                span,
                module: QuoteWat::Wat(Wat::Module(module)),
                message,
            } => {
                test_context.profile().bump_assert_malformed();
                module_compilation_fails(test_context, span, module, message);
            }
            WastDirective::AssertMalformed { .. } => {
                test_context.profile().bump_assert_malformed();
            }
            WastDirective::AssertInvalid {
                span,
                module,
                message,
            } => {
                test_context.profile().bump_assert_invalid();
                let module = match extract_module(module) {
                    Some(module) => module,
                    None => continue 'outer,
                };
                module_compilation_fails(test_context, span, module, message);
            }
            WastDirective::Register { span, name, module } => {
                test_context.profile().bump_register();
                let module_name = module.map(|id| id.name());
                let instance = test_context
                    .instance_by_name_or_last(module_name)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{}: failed to load module: {}",
                            test_context.spanned(span),
                            error
                        )
                    });
                test_context.register_instance(name, instance);
            }
            WastDirective::Invoke(wast_invoke) => {
                let span = wast_invoke.span;
                test_context.profile().bump_invoke();
                execute_wast_invoke(test_context, span, wast_invoke).unwrap_or_else(|error| {
                    panic!(
                        "{}: failed to invoke `.wast` directive: {}",
                        test_context.spanned(span),
                        error
                    )
                });
            }
            WastDirective::AssertTrap {
                span,
                exec,
                message,
            } => {
                test_context.profile().bump_assert_trap();
                match execute_wast_execute(test_context, span, exec) {
                    Ok(results) => panic!(
                        "{}: expected to trap with message '{}' but succeeded with: {:?}",
                        test_context.spanned(span),
                        message,
                        results
                    ),
                    Err(error) => assert_trap(test_context, span, error, message),
                }
            }
            WastDirective::AssertReturn {
                span,
                exec,
                results: expected,
            } => {
                test_context.profile().bump_assert_return();
                let results =
                    execute_wast_execute(test_context, span, exec).unwrap_or_else(|error| {
                        panic!(
                            "{}: encountered unexpected failure to execute `AssertReturn`: {}",
                            test_context.spanned(span),
                            error
                        )
                    });
                assert_results(test_context, span, &results, &expected);
            }
            WastDirective::AssertExhaustion {
                span,
                call,
                message,
            } => {
                test_context.profile().bump_assert_exhaustion();
                match execute_wast_invoke(test_context, span, call) {
                    Ok(results) => {
                        panic!(
                            "{}: expected to fail due to resource exhaustion '{}' but succeeded with: {:?}",
                            test_context.spanned(span),
                            message,
                            results
                        )
                    }
                    Err(error) => assert_trap(test_context, span, error, message),
                }
            }
            WastDirective::AssertUnlinkable {
                span,
                module: Wat::Module(module),
                message,
            } => {
                test_context.profile().bump_assert_unlinkable();
                module_compilation_fails(test_context, span, module, message);
            }
            WastDirective::AssertUnlinkable { .. } => {
                test_context.profile().bump_assert_unlinkable();
            }
            WastDirective::AssertException { span, exec } => {
                test_context.profile().bump_assert_exception();
                if let Ok(results) = execute_wast_execute(test_context, span, exec) {
                    panic!(
                        "{}: expected to fail due to exception but succeeded with: {:?}",
                        test_context.spanned(span),
                        results
                    )
                }
            }
        }
    }
    Ok(())
}

/// Asserts that the `error` is a trap with the expected `message`.
///
/// # Panics
///
/// - If the `error` is not a trap.
/// - If the trap message of the `error` is not as expected.
fn assert_trap(test_context: &TestContext, span: Span, error: TestError, message: &str) {
    match error {
        TestError::Wasmi(error) => {
            assert!(
                error.to_string().starts_with(message),
                "{}: the directive trapped as expected but with an unexpected message\n\
                    expected: {},\n\
                    encountered: {}",
                test_context.spanned(span),
                message,
                error,
            );
        }
        unexpected => panic!(
            "encountered unexpected error: \n\t\
                found: '{unexpected}'\n\t\
                expected: trap with message '{message}'",
        ),
    }
}

/// Asserts that `results` match the `expected` values.
fn assert_results(context: &TestContext, span: Span, results: &[Value], expected: &[WastRet]) {
    assert_eq!(results.len(), expected.len());
    let expected = expected.iter().map(|expected| match expected {
        WastRet::Core(expected) => expected,
        WastRet::Component(expected) => panic!(
            "`wasmi` does not support the Wasm `component-model` proposal but found {expected:?}"
        ),
    });
    for (result, expected) in results.iter().zip(expected) {
        match (result, expected) {
            (Value::I32(result), WastRetCore::I32(expected)) => {
                assert_eq!(result, expected, "in {}", context.spanned(span))
            }
            (Value::I64(result), WastRetCore::I64(expected)) => {
                assert_eq!(result, expected, "in {}", context.spanned(span))
            }
            (Value::F32(result), WastRetCore::F32(expected)) => match expected {
                NanPattern::CanonicalNan | NanPattern::ArithmeticNan => assert!(result.is_nan()),
                NanPattern::Value(expected) => {
                    assert_eq!(
                        result.to_bits(),
                        expected.bits,
                        "in {}",
                        context.spanned(span)
                    );
                }
            },
            (Value::F64(result), WastRetCore::F64(expected)) => match expected {
                NanPattern::CanonicalNan | NanPattern::ArithmeticNan => {
                    assert!(result.is_nan(), "in {}", context.spanned(span))
                }
                NanPattern::Value(expected) => {
                    assert_eq!(
                        result.to_bits(),
                        expected.bits,
                        "in {}",
                        context.spanned(span)
                    );
                }
            },
            (result, expected) => panic!(
                "{}: encountered mismatch in evaluation. expected {:?} but found {:?}",
                context.spanned(span),
                expected,
                result
            ),
        }
    }
}

fn extract_module(quote_wat: QuoteWat) -> Option<wast::core::Module> {
    match quote_wat {
        QuoteWat::Wat(Wat::Module(module)) => Some(module),
        QuoteWat::Wat(Wat::Component(_))
        | QuoteWat::QuoteModule(_, _)
        | QuoteWat::QuoteComponent(_, _) => {
            // We currently do not allow parsing `.wat` Wasm modules in `v1`
            // therefore checks based on malformed `.wat` modules are uninteresting
            // to us at the moment.
            // This might become interesting once `v1` starts support parsing `.wat`
            // Wasm modules.
            None
        }
    }
}

fn module_compilation_fails(
    context: &mut TestContext,
    span: Span,
    module: wast::core::Module,
    expected_message: &str,
) {
    let result = context.compile_and_instantiate(module);
    assert!(
        result.is_err(),
        "{}: succeeded to instantiate module but should have failed with: {}",
        context.spanned(span),
        expected_message
    );
}

fn execute_wast_execute(
    context: &mut TestContext,
    span: Span,
    execute: WastExecute,
) -> Result<Vec<Value>, TestError> {
    match execute {
        WastExecute::Invoke(invoke) => {
            execute_wast_invoke(context, span, invoke).map_err(Into::into)
        }
        WastExecute::Wat(Wat::Module(module)) => {
            context.compile_and_instantiate(module).map(|_| Vec::new())
        }
        WastExecute::Wat(Wat::Component(_)) => {
            // Wasmi currently does not support the Wasm component model.
            Ok(vec![])
        }
        WastExecute::Get { module, global } => context
            .get_global(module, global)
            .map(|result| vec![result]),
    }
}

fn execute_wast_invoke(
    context: &mut TestContext,
    span: Span,
    invoke: WastInvoke,
) -> Result<Vec<Value>, TestError> {
    let module_name = invoke.module.map(|id| id.name());
    let field_name = invoke.name;
    let mut args = <Vec<Value>>::new();
    for arg in invoke.args {
        let value = match arg {
            wast::WastArg::Core(arg) => {
                match arg {
                    wast::core::WastArgCore::I32(arg) => Value::I32(arg),
                    wast::core::WastArgCore::I64(arg) => Value::I64(arg),
                    wast::core::WastArgCore::F32(arg) => Value::F32(F32::from_bits(arg.bits)),
                    wast::core::WastArgCore::F64(arg) => Value::F64(F64::from_bits(arg.bits)),
                    wast::core::WastArgCore::V128(arg) => panic!("{span:?}: `wasmi` does not support the `simd` Wasm proposal but found: {arg:?}"),
                    wast::core::WastArgCore::RefNull(_) |
                    wast::core::WastArgCore::RefExtern(_) => panic!("{span:?}: `wasmi` does not support the `reference-types` Wasm proposal but found {arg:?}"),
                }
            }
            wast::WastArg::Component(arg) => panic!("{span:?}: `wasmi` does not support the Wasm `component-model` but found {arg:?}"),
        };
        args.push(value);
    }
    context
        .invoke(module_name, field_name, &args)
        .map(|results| results.to_vec())
}
