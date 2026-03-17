use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: run_bundle <bundle_or_jar>... <main_class> <method> <descriptor>");
        eprintln!("  Files ending in .jar are loaded via the JAR loader.");
        eprintln!("  All other files are treated as flat bundle format.");
        std::process::exit(2);
    }
    // Last 3 args are class, method, descriptor; everything before is input files
    let input_files = &args[1..args.len() - 3];
    let main_class = &args[args.len() - 3];
    let method = &args[args.len() - 2];
    let descriptor = &args[args.len() - 1];

    let mut vm = jvm_core::interpreter::Vm::new();

    for path in input_files {
        let data = fs::read(path).unwrap_or_else(|e| {
            eprintln!("failed to read {}: {}", path, e);
            std::process::exit(2);
        });
        if path.ends_with(".jar") {
            match vm.load_jar(&data) {
                Ok(count) => eprintln!("Loaded JAR {}: {} classes", path, count),
                Err(e) => {
                    eprintln!("failed to load JAR {}: {}", path, e);
                    std::process::exit(2);
                }
            }
        } else {
            jvm_core::load_bundle(&mut vm, &data);
        }
    }

    let result = vm.invoke_static_threaded(main_class, method, descriptor, vec![]);
    vm.flush_printstreams();
    match result {
        Ok(v) => {
            let s = match &v {
                jvm_core::heap::JValue::Ref(Some(r)) => {
                    let is_str = matches!(r.borrow().native, jvm_core::heap::NativePayload::JavaString(_));
                    if is_str {
                        r.borrow().as_java_string().unwrap_or_default().to_owned()
                    } else {
                        let cn = r.borrow().class_name.clone();
                        match vm.invoke_virtual(r.clone(), &cn, "toString", "()Ljava/lang/String;", vec![]) {
                            Ok(jvm_core::heap::JValue::Ref(Some(s))) => {
                                s.borrow().as_java_string().unwrap_or_default().to_owned()
                            }
                            _ => format!("{}@obj", cn),
                        }
                    }
                }
                jvm_core::heap::JValue::Void => "void".to_owned(),
                jvm_core::heap::JValue::Int(i) => i.to_string(),
                jvm_core::heap::JValue::Long(l) => l.to_string(),
                jvm_core::heap::JValue::Float(f) => f.to_string(),
                jvm_core::heap::JValue::Double(d) => d.to_string(),
                jvm_core::heap::JValue::Ref(None) => "null".to_owned(),
                _ => format!("{:?}", v),
            };
            println!("{s}");
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
