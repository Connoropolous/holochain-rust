// In this example we execute a contract funciton exported as "_call"

use wasmi::{
    self, Error as InterpreterError, Externals, FuncInstance, FuncRef, ImportsBuilder,
    ModuleImportResolver, ModuleInstance, RuntimeArgs, RuntimeValue, Signature, Trap, ValueType,
};

#[derive(Clone)]
pub struct Runtime {
    print_output: Vec<u32>,
    pub result: String,
}

pub const RESULT_OFFSET: u32 = 0;

#[allow(dead_code)]
pub fn call(
    wasm: Vec<u8>,
    function_name: &str,
    parameters: Option<Vec<u8>>,
) -> Result<Runtime, InterpreterError> {
    let module = wasmi::Module::from_buffer(wasm).unwrap();

    const PRINT_FUNC_INDEX: usize = 0;

    impl Externals for Runtime {
        fn invoke_index(
            &mut self,
            index: usize,
            args: RuntimeArgs,
        ) -> Result<Option<RuntimeValue>, Trap> {
            match index {
                PRINT_FUNC_INDEX => {
                    let arg: u32 = args.nth(0);
                    self.print_output.push(arg);
                    Ok(None)
                }
                _ => panic!("unknown function index"),
            }
        }
    }

    struct RuntimeModuleImportResolver;

    impl ModuleImportResolver for RuntimeModuleImportResolver {
        fn resolve_func(
            &self,
            field_name: &str,
            _signature: &Signature,
        ) -> Result<FuncRef, InterpreterError> {
            let func_ref = match field_name {
                "print" => FuncInstance::alloc_host(
                    Signature::new(&[ValueType::I32][..], None),
                    PRINT_FUNC_INDEX,
                ),
                _ => {
                    return Err(InterpreterError::Function(format!(
                        "host module doesn't export function with name {}",
                        field_name
                    )))
                }
            };
            Ok(func_ref)
        }
    }

    let mut imports = ImportsBuilder::new();
    imports.push_resolver("env", &RuntimeModuleImportResolver);

    let main = ModuleInstance::new(&module, &imports)
        .expect("Failed to instantiate module")
        .assert_no_start();

    let memory = main
        .export_by_name("memory")
        .expect("all modules compiled with rustc should have an export named 'memory'; qed")
        .as_memory()
        .expect("in module generated by rustc export named 'memory' should be a memory; qed")
        .clone();

    let params: Vec<_> = parameters.unwrap_or_default();

    memory.set(0, &params).expect("memory should be writable");

    let mut runtime = Runtime {
        print_output: vec![],
        result: String::new(),
    };

    let i32_result_length: i32 = main
        .invoke_export(
            format!("{}_dispatch", function_name).as_str(),
            &[RuntimeValue::I32(0), RuntimeValue::I32(params.len() as i32)],
            &mut runtime,
        )?
        .unwrap()
        .try_into()
        .unwrap();

    let result = memory
        .get(RESULT_OFFSET, i32_result_length as usize)
        .expect("Successfully retrieve the result");
    runtime.result = String::from_utf8(result).unwrap();
    Ok(runtime.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wabt::Wat2Wasm;

    fn test_wasm() -> Vec<u8> {
        let wasm_binary = Wat2Wasm::new()
            .canonicalize_lebs(false)
            .write_debug_names(true)
            .convert(
                r#"
                (module
                    (type (;0;) (func (result i32)))
                    (type (;1;) (func (param i32)))
                    (type (;2;) (func))
                    (import "env" "print" (func $print (type 1)))
                    (func (export "test_print_dispatch") (param $p0 i32) (param $p1 i32) (result i32)
                        i32.const 1337
                        call $print
                        i32.const 0)
                    (func $rust_eh_personality (type 2))
                    (table (;0;) 1 1 anyfunc)
                    (memory (;0;) 17)
                    (global (;0;) (mut i32) (i32.const 1049600))
                    (export "memory" (memory 0))
                    (export "rust_eh_personality" (func $rust_eh_personality)))
            "#,
            )
            .unwrap();

        wasm_binary.as_ref().to_vec()
    }

    #[test]
    fn test_print() {
        let runtime = call(test_wasm(), "test_print", None).expect("test_print should be callable");
        assert_eq!(runtime.print_output.len(), 1);
        assert_eq!(runtime.print_output[0], 1337)
    }
}