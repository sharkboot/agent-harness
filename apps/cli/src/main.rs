use std::process;
use std::io;

fn main() {
    // 解析命令行参数
    let args: Vec<String> = process::args().collect();

    if args.contains(&"--headless".to_string()) {
        // Headless 模式：输出到 stdout
        println!("Agent Harness Headless CLI");
        println!("Running in headless mode...");

        // 读取 stdin 或参数
        let input = args.get(3).cloned().unwrap_or_default();
        if !input.is_empty() {
            println!("Input: {}", input);
        } else {
            let mut buffer = String::new();
            io::stdin().read_line(&mut buffer).ok();
            let input = buffer.trim();
            if !input.is_empty() {
                println!("Input: {}", input);
            }
        }

        // 模拟响应
        println!("Mock response: Received input");
        return;
    }

    // Interactive 模式
    println!("Agent Harness TUI v0.1.0");
    println!("Type 'quit' to exit");
    println!();

    loop {
        print!("> ");
        io::stdout().flush().ok();

        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap_or(0);
        let input = buffer.trim();

        match input {
            "" => continue,
            "quit" | "exit" | "q" => {
                println!("Bye!");
                break;
            }
            _ => {
                // 模拟 Agent 响应
                println!(
                    "[Agent] You said: {}\n[Agent] Mock response: I received your message.",
                    input
                );
                println!();
            }
        }
    }
}
