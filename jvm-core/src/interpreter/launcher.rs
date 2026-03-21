use crate::heap::{JObject, JValue};

use super::{StdioMode, ThreadState, Vm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessExit {
    pub exit_code: i32,
    pub uncaught_exception: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    WaitingForInput,
    Exited,
}

pub struct JvmProcess {
    vm: Vm,
    exit: Option<ProcessExit>,
}

fn parse_system_exit(message: &str) -> Option<i32> {
    let marker = "System.exit(";
    let start = message.find(marker)? + marker.len();
    let end = message[start..].find(')')?;
    message[start..start + end].parse().ok()
}

impl JvmProcess {
    pub fn launch_classpath_main(
        shim_bundle: &[u8],
        jar_data: &[u8],
        main_class: &str,
        args: &[String],
        stdin: StdioMode,
        stdout: StdioMode,
        stderr: StdioMode,
    ) -> Result<Self, String> {
        let mut vm = Vm::new();
        crate::load_bundle(&mut vm, shim_bundle);
        crate::load_jars(&mut vm, jar_data)
            .map_err(|e| format!("launchClasspathMain failed during classpath load: {e}"))?;
        vm.set_stdio_modes(stdin, stdout, stderr);
        vm.scheduler = super::Scheduler::new();
        vm.monitors.clear();

        let arg_values = args
            .iter()
            .map(|arg| JValue::Ref(Some(vm.intern_string(arg.clone()))))
            .collect();
        let args_array = JObject::new_array("[Ljava/lang/String;", arg_values);
        let main_args = vec![JValue::Ref(Some(args_array))];

        let exit = match vm.build_static_frame(
            main_class,
            "main",
            "([Ljava/lang/String;)V",
            main_args.clone(),
            true,
        )? {
            Some(frame) => {
                vm.scheduler.current_thread_mut().call_stack.push(frame);
                None
            }
            None => {
                if vm
                    .native_static(main_class, "main", "([Ljava/lang/String;)V", &main_args)
                    .is_some()
                {
                    if let Some(err) = vm.pending_exception_err() {
                        Some(ProcessExit {
                            exit_code: 1,
                            uncaught_exception: Some(err),
                        })
                    } else {
                        Some(ProcessExit {
                            exit_code: 0,
                            uncaught_exception: None,
                        })
                    }
                } else {
                    return Err(format!(
                        "Method not found: {main_class}.main([Ljava/lang/String;)V"
                    ));
                }
            }
        };

        if exit.is_some() {
            vm.flush_printstreams();
        }

        Ok(Self { vm, exit })
    }

    pub fn pump(&mut self, max_rounds: usize) -> ProcessState {
        if self.exit.is_some() {
            return ProcessState::Exited;
        }
        if max_rounds == 0 {
            return ProcessState::Running;
        }

        let mut rounds = 0usize;
        while rounds < max_rounds {
            rounds += 1;
            let current_id = self.vm.scheduler.current_thread().id;
            let state = self.vm.scheduler.current_thread().state;

            match state {
                ThreadState::Runnable => {
                    let mut call_stack =
                        std::mem::take(&mut self.vm.scheduler.current_thread_mut().call_stack);
                    let time_slice = self.vm.effective_time_slice();
                    let result = self.vm.run_trampoline_steps(&mut call_stack, time_slice);
                    self.vm.scheduler.current_thread_mut().call_stack = call_stack;

                    match result {
                        Ok(Some(_)) => {
                            self.vm.scheduler.current_thread_mut().state = ThreadState::Terminated;
                        }
                        Ok(None) => {
                            let thread = self.vm.scheduler.current_thread_mut();
                            match thread.state {
                                ThreadState::Yielded | ThreadState::Sleeping => {
                                    thread.state = ThreadState::Runnable;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            self.vm.scheduler.current_thread_mut().state = ThreadState::Terminated;
                            if current_id == 0 {
                                let err = self.vm.pending_exception_err().unwrap_or(e);
                                if let Some(code) = parse_system_exit(&err) {
                                    self.finish(ProcessExit {
                                        exit_code: code,
                                        uncaught_exception: None,
                                    });
                                    return ProcessState::Exited;
                                }
                                self.finish(ProcessExit {
                                    exit_code: 1,
                                    uncaught_exception: Some(err),
                                });
                                return ProcessState::Exited;
                            }
                        }
                    }
                }
                ThreadState::Terminated => {}
                ThreadState::Yielded | ThreadState::Sleeping => {
                    self.vm.scheduler.current_thread_mut().state = ThreadState::Runnable;
                }
                ThreadState::Joining(_)
                | ThreadState::WaitingOnMonitor(_)
                | ThreadState::WaitingOnCondition(_) => {}
            }

            self.vm.scheduler.wake_joiners();

            if self.vm.scheduler.all_terminated() {
                self.finish(ProcessExit {
                    exit_code: 0,
                    uncaught_exception: None,
                });
                return ProcessState::Exited;
            }

            if !self.vm.scheduler.advance() {
                if self.vm.scheduler.all_terminated() {
                    self.finish(ProcessExit {
                        exit_code: 0,
                        uncaught_exception: None,
                    });
                    return ProcessState::Exited;
                }
                if self.vm.scheduler.runnable_count() == 0 {
                    if self.vm.is_waiting_on_stdin() {
                        return ProcessState::WaitingForInput;
                    }
                    self.finish(ProcessExit {
                        exit_code: 1,
                        uncaught_exception: Some(format!(
                            "Deadlock: no runnable threads ({})",
                            self.vm.scheduler.alive_thread_summary()
                        )),
                    });
                    return ProcessState::Exited;
                }
            }
        }

        ProcessState::Running
    }

    pub fn write_stdin(&mut self, bytes: &[u8]) {
        if self.exit.is_none() {
            self.vm.write_stdin(bytes);
        }
    }

    pub fn close_stdin(&mut self) {
        if self.exit.is_none() {
            self.vm.close_stdin();
        }
    }

    pub fn take_stdout(&mut self) -> Vec<u8> {
        self.vm.take_stdout()
    }

    pub fn take_stderr(&mut self) -> Vec<u8> {
        self.vm.take_stderr()
    }

    pub fn kill(&mut self) {
        if self.exit.is_none() {
            self.finish(ProcessExit {
                exit_code: 137,
                uncaught_exception: Some("Process killed".to_owned()),
            });
        }
    }

    pub fn exit(&self) -> Option<&ProcessExit> {
        self.exit.as_ref()
    }

    pub fn is_exited(&self) -> bool {
        self.exit.is_some()
    }

    fn finish(&mut self, exit: ProcessExit) {
        self.vm.flush_printstreams();
        self.vm.scheduler.reset_to_main();
        self.exit = Some(exit);
    }
}
