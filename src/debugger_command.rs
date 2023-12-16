pub enum DebuggerCommand {
    Quit,
    Step,
    Run(Vec<String>),
    Continue,
    Backtrace,
    Breakpoint(String),
}

impl DebuggerCommand {
    pub fn from_tokens(tokens: &Vec<&str>) -> Option<Self> {
        match tokens[0] {
            "q"  | "quit" | "exit"   => Some(DebuggerCommand::Quit),
            "s"  | "step" | "next"   => Some(DebuggerCommand::Step),
            "c"  | "cont" | "continue"   => Some(DebuggerCommand::Continue),
            "bt" | "back" | "backtrace"  => Some(DebuggerCommand::Backtrace),
            "b"  | "break"| "breakpoint" => Some(DebuggerCommand::Breakpoint(tokens[1].to_string())),
            "r"  | "run"   => {
                let args = tokens[1..].to_vec();
                Some(DebuggerCommand::Run(
                    args.iter().map(|s| s.to_string()).collect(),
                ))
            },

            _ => None,
        }
    }
}
